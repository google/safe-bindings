// Copyright 2026 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![forbid(unsafe_code)]

//! Fast, memory-safe JSON sanitizer adapter for `serde_json`.
//!
//! Standard JSON (RFC 8259) does not allow comments or trailing commas in arrays
//! or objects. Many human-authored configuration files (such as JSON5 or JSONC)
//! include single-line comments (`// ...`), multi-line comments (`/* ... */`),
//! and trailing commas (e.g. `[1, 2,]` or `{"a": 1,}`).
//!
//! `serde_json` strictly rejects these inputs by default.
//!
//! [`JsonSanitizer`] is a zero-copy streaming [`std::io::Read`] adapter that wraps
//! any underlying reader and filters out:
//! 1. **Single-line comments**: `// ...` up to a newline or EOF.
//! 2. **Multi-line comments**: `/* ... */`.
//! 3. **Trailing commas**: Commas in arrays (`[...]`) or objects (`{...}`) that
//!    are followed only by whitespace and/or comments before the closing bracket
//!    or brace.
//!
//! # How it works
//!
//! Rather than shifting bytes or rebuilding an AST, [`JsonSanitizer`] replaces
//! comment characters and trailing commas with ASCII spaces (`0x20`), while
//! preserving all line terminators (`\n`, `\r\n`).
//!
//! This design provides major advantages:
//! - **1:1 Byte Mapping**: The output length exactly matches the input length.
//! - **Accurate Error Reporting**: Line and column numbers in `serde_json` error
//!   messages match the original document byte-for-byte.
//! - **Token Isolation**: Comments in JS/JSON5 act as whitespace; replacing them
//!   with whitespace guarantees that adjacent tokens (e.g., `true/*c*/false`)
//!   do not merge into invalid or unintended tokens.
//! - **High Performance**: In-place replacements avoid memory reallocations,
//!   buffer copies, or string parsing overhead.
//! - **Buffered Streaming**: Amortizes byte-by-byte reads from
//!   `serde_json::from_reader` by refilling in 16 KiB chunks and enables
//!   lookahead for trailing comma and comment resolution.
//!
//! # Example
//!
//! ```rust
//! use json_sanitizer::JsonSanitizer;
//! use std::io::Cursor;
//!
//! let json5_data = r#"
//! {
//!     // Server configuration
//!     "host": "localhost",
//!     "port": 8080, /* default port */
//!     "endpoints": [
//!         "/api/v1",
//!         "/api/v2", // trailing comma below:
//!     ],
//! }
//! "#;
//!
//! let reader = Cursor::new(json5_data.as_bytes());
//! let sanitizer = JsonSanitizer::new(reader);
//! let parsed: serde_json::Value = serde_json::from_reader(sanitizer).unwrap();
//!
//! assert_eq!(parsed["host"], "localhost");
//! assert_eq!(parsed["port"], 8080);
//! assert_eq!(parsed["endpoints"].as_array().unwrap().len(), 2);
//! ```

use std::fmt;
use std::io::{self, BufRead, Read};

/// Default capacity for the internal read buffer (16 KiB).
const DEFAULT_BUF_SIZE: usize = 16 * 1024;

/// Default maximum capacity limit for the internal read buffer (16 KiB).
const DEFAULT_MAX_BUF_SIZE: usize = DEFAULT_BUF_SIZE;

/// Internal state of the JSON lexer / state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum State {
    /// Normal JSON stream outside strings and comments.
    #[default]
    Normal,
    /// Inside a double-quoted string: `"..."`.
    InString,
    /// Immediately after a backslash escape inside a string: `\`.
    InStringEscape,
    /// Saw a `/` in [`State::Normal`]. Waiting for next byte (`/`, `*`, or other).
    SawSlash,
    /// Inside a single-line comment: `// ...`.
    InLineComment,
    /// Inside a multi-line comment: `/* ... */`.
    InBlockComment,
    /// Inside a multi-line comment immediately after a `*`. Waiting for `/` to close.
    InBlockCommentSawStar,
}

/// Tracks whether the last non-whitespace, non-comment token was a completed value.
///
/// In JSON, a trailing comma can only occur *after* an element/value has completed
/// (e.g., after `"..."`, a number, `true`, `false`, `null`, `]`, or `}`).
///
/// A comma immediately following `[`, `{`, `,`, or `:` is a leading comma or double comma,
/// which is a syntax error in JSON and must NOT be converted to a space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PrecedingToken {
    /// At start of input, or immediately after `[`, `{`, `,`, or `:`.
    #[default]
    NotValue,
    /// Immediately after a completed value (string, number, boolean, null, `]`, or `}`).
    Value,
}

/// Streaming [`Read`] and [`BufRead`] adapter that strips comments and trailing commas from a JSON stream.
///
/// Wraps an underlying [`Read`] source and performs buffered, streaming sanitization.
///
/// # Buffer Capacity and Memory Usage
///
/// The adapter maintains an internal buffer (default 16 KiB) to amortize reads from the
/// underlying source and to support lookahead. Under normal streaming, unconsumed bytes
/// are compacted and memory usage remains constant at 16 KiB.
///
/// Memory consumption is strictly bounded by a maximum capacity limit (default 16 KiB,
/// configurable via [`JsonSanitizer::with_max_capacity`]). If trailing comma lookahead
/// cannot resolve within this upper limit—for example, if a comma is followed by more
/// than 16 KiB of continuous comments and/or whitespace before the next token or closing
/// delimiter—the lookahead window closes gracefully: the pending comma is preserved as a
/// regular comma rather than allocating additional memory. This guarantees that internal
/// buffer memory will never grow unbounded even on adversarial or oversized inputs.
pub struct JsonSanitizer<S> {
    inner: S,
    buffer: Vec<u8>,
    max_capacity: usize,
    head: usize,
    state: State,
    depth: usize,
    preceding_token: PrecedingToken,
    pending_comma_offset: Option<usize>,
    eof: bool,
}

impl<S: fmt::Debug> fmt::Debug for JsonSanitizer<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JsonSanitizer")
            .field("inner", &self.inner)
            .field("buffer_capacity", &self.buffer.capacity())
            .field("max_capacity", &self.max_capacity)
            .field("buffered_bytes", &(self.buffer.len() - self.head))
            .field("state", &self.state)
            .field("depth", &self.depth)
            .field("preceding_token", &self.preceding_token)
            .field("pending_comma", &self.pending_comma_offset.is_some())
            .field("eof", &self.eof)
            .finish()
    }
}

impl<S> JsonSanitizer<S> {
    /// Creates a new `JsonSanitizer` wrapping `inner` with the default buffer capacity (16 KiB)
    /// and default maximum capacity limit (16 KiB).
    pub fn new(inner: S) -> Self {
        Self::with_capacity(inner, DEFAULT_BUF_SIZE)
    }

    /// Creates a new `JsonSanitizer` wrapping `inner` with a specified buffer capacity.
    ///
    /// The maximum capacity limit defaults to `std::cmp::max(capacity, DEFAULT_MAX_BUF_SIZE)`.
    pub fn with_capacity(inner: S, capacity: usize) -> Self {
        let max_cap = std::cmp::max(capacity, DEFAULT_MAX_BUF_SIZE);
        Self::with_max_capacity(inner, capacity, max_cap)
    }

    /// Creates a new `JsonSanitizer` wrapping `inner` with a specified initial buffer capacity
    /// and maximum buffer capacity limit.
    pub fn with_max_capacity(inner: S, capacity: usize, max_capacity: usize) -> Self {
        let cap = std::cmp::max(capacity, 64);
        let max_cap = std::cmp::max(max_capacity, cap);
        Self {
            inner,
            buffer: Vec::with_capacity(cap),
            max_capacity: max_cap,
            head: 0,
            state: State::Normal,
            depth: 0,
            preceding_token: PrecedingToken::NotValue,
            pending_comma_offset: None,
            eof: false,
        }
    }

    /// Returns the current capacity of the internal buffer.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    /// Returns the maximum allowed capacity of the internal buffer.
    #[inline]
    pub fn max_capacity(&self) -> usize {
        self.max_capacity
    }
}

impl<S: Read> Read for JsonSanitizer<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        // Fast path for 1-byte reads (frequently used by serde_json::from_reader).
        let ready_end = self.ready_end();
        if self.head < ready_end {
            if buf.len() == 1 {
                buf[0] = self.buffer[self.head];
                self.head += 1;
                return Ok(1);
            }
            let to_copy = std::cmp::min(buf.len(), ready_end - self.head);
            buf[..to_copy].copy_from_slice(&self.buffer[self.head..self.head + to_copy]);
            self.head += to_copy;
            return Ok(to_copy);
        }

        let available = self.fill_buf()?;
        if available.is_empty() {
            return Ok(0);
        }
        let to_copy = std::cmp::min(buf.len(), available.len());
        buf[..to_copy].copy_from_slice(&available[..to_copy]);
        self.consume(to_copy);
        Ok(to_copy)
    }
}

impl<S: Read> BufRead for JsonSanitizer<S> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        loop {
            let ready_end = self.ready_end();
            if self.head < ready_end {
                return Ok(&self.buffer[self.head..ready_end]);
            }

            if self.eof {
                // If EOF is reached, check for unclosed multi-line comments.
                if self.state == State::InBlockComment || self.state == State::InBlockCommentSawStar
                {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "unclosed multi-line comment in JSON",
                    ));
                }
                return Ok(&[]);
            }

            self.refill()?;
        }
    }

    fn consume(&mut self, amt: usize) {
        let ready_end = self.ready_end();
        self.head = std::cmp::min(self.head + amt, ready_end);
    }
}

impl<S: Read> JsonSanitizer<S> {
    /// Returns the index in `self.buffer` up to which bytes are finalized and safe to emit.
    #[inline]
    fn ready_end(&self) -> usize {
        if self.eof {
            return self.buffer.len();
        }

        match self.pending_comma_offset {
            Some(pos) => pos,
            None => {
                if self.state == State::SawSlash {
                    self.buffer.len().saturating_sub(1)
                } else {
                    self.buffer.len()
                }
            }
        }
    }

    /// Compacts unread bytes and refills `self.buffer` from `self.inner`.
    fn refill(&mut self) -> io::Result<()> {
        // 1. Slide unconsumed bytes to front.
        if self.head > 0 {
            let remaining = self.buffer.len() - self.head;
            self.buffer.copy_within(self.head.., 0);
            self.buffer.truncate(remaining);
            if let Some(pos) = self.pending_comma_offset.as_mut() {
                *pos -= self.head;
            }
            self.head = 0;
        }

        // 2. Ensure spare capacity to read from `inner`.
        if self.buffer.len() == self.buffer.capacity() {
            if self.buffer.capacity() < self.max_capacity {
                let target_cap =
                    std::cmp::min(self.buffer.capacity().saturating_mul(2), self.max_capacity);
                let additional = target_cap - self.buffer.len();
                self.buffer.reserve_exact(additional);
            } else {
                // Buffer has reached max_capacity and cannot resolve the lookahead.
                // Finalize the pending comma: treat it as a normal comma rather than
                // allocating additional memory.
                if self.pending_comma_offset.is_some() {
                    self.pending_comma_offset = None;
                    return Ok(());
                }
                return Ok(());
            }
        }

        // 3. Read into spare capacity.
        let prev_len = self.buffer.len();
        let cap = self.buffer.capacity();
        self.buffer.resize(cap, 0);

        match self.inner.read(&mut self.buffer[prev_len..cap]) {
            Ok(0) => {
                // End of file from `inner`.
                self.buffer.truncate(prev_len);
                self.eof = true;

                // Finalize any pending comma (it stays a comma at EOF).
                self.pending_comma_offset = None;

                // Finalize any trailing slash at EOF (remains a bare slash).
                if self.state == State::SawSlash {
                    self.state = State::Normal;
                }

                Ok(())
            }
            Ok(n) => {
                self.buffer.truncate(prev_len + n);
                process_chunk(
                    &mut self.buffer,
                    prev_len,
                    &mut self.state,
                    &mut self.depth,
                    &mut self.preceding_token,
                    &mut self.pending_comma_offset,
                );
                Ok(())
            }
            Err(e) => {
                self.buffer.truncate(prev_len);
                Err(e)
            }
        }
    }
}

/// Sanitizes a chunk of bytes in `buffer[start..]`, updating the state machine.
///
/// Note: `buffer` passed here is the full buffer slice so that `pending_comma_offset`
/// (which might refer to an index `< start`) and the slash before `start` (for `SawSlash`)
/// can be directly mutated in-place.
#[inline]
fn process_chunk(
    buffer: &mut [u8],
    start: usize,
    state: &mut State,
    depth: &mut usize,
    preceding_token: &mut PrecedingToken,
    pending_comma_offset: &mut Option<usize>,
) {
    let end = buffer.len();
    let mut i = start;

    while i < end {
        let b = buffer[i];
        match *state {
            State::Normal => match b {
                b'"' => {
                    if pending_comma_offset.is_some() {
                        // String token follows comma; comma was NOT trailing.
                        *pending_comma_offset = None;
                    }
                    *preceding_token = PrecedingToken::Value;
                    *state = State::InString;
                    i += 1;

                    // Fast inner loop for skipping string content.
                    while i < end {
                        let sb = buffer[i];
                        if sb == b'"' {
                            *state = State::Normal;
                            i += 1;
                            break;
                        } else if sb == b'\\' {
                            *state = State::InStringEscape;
                            i += 1;
                            break;
                        }
                        i += 1;
                    }
                    continue;
                }
                b'/' => {
                    *state = State::SawSlash;
                }
                b',' => {
                    if pending_comma_offset.is_some() {
                        // Double comma: previous comma followed by ',', not ']' or '}'.
                        *pending_comma_offset = None;
                    }
                    if *depth > 0 && *preceding_token == PrecedingToken::Value {
                        *pending_comma_offset = Some(i);
                    }
                    *preceding_token = PrecedingToken::NotValue;
                }
                b']' | b'}' => {
                    *depth = depth.saturating_sub(1);
                    if let Some(pos) = pending_comma_offset.take() {
                        // Trailing comma found! Replace with space.
                        buffer[pos] = b' ';
                    }
                    *preceding_token = PrecedingToken::Value;
                }
                b'[' | b'{' => {
                    *depth += 1;
                    if pending_comma_offset.is_some() {
                        *pending_comma_offset = None;
                    }
                    *preceding_token = PrecedingToken::NotValue;
                }
                b':' => {
                    if pending_comma_offset.is_some() {
                        *pending_comma_offset = None;
                    }
                    *preceding_token = PrecedingToken::NotValue;
                }
                b' ' | b'\t' | b'\n' | b'\r' => {
                    // Whitespace: does not resolve pending comma, does not change preceding_token.
                }
                _ => {
                    // Any other character (e.g. '0'..='9', '-', 't', 'f', 'n'...)
                    if pending_comma_offset.is_some() {
                        *pending_comma_offset = None;
                    }
                    *preceding_token = PrecedingToken::Value;
                }
            },
            State::InString => {
                while i < end {
                    let sb = buffer[i];
                    if sb == b'"' {
                        *state = State::Normal;
                        i += 1;
                        break;
                    } else if sb == b'\\' {
                        *state = State::InStringEscape;
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            State::InStringEscape => {
                // Any character immediately following `\` inside a string is escaped.
                *state = State::InString;
            }
            State::SawSlash => match b {
                b'/' => {
                    buffer[i - 1] = b' ';
                    buffer[i] = b' ';
                    *state = State::InLineComment;
                }
                b'*' => {
                    buffer[i - 1] = b' ';
                    buffer[i] = b' ';
                    *state = State::InBlockComment;
                }
                _ => {
                    // Bare slash! Not a comment.
                    if pending_comma_offset.is_some() {
                        *pending_comma_offset = None;
                    }
                    *preceding_token = PrecedingToken::NotValue;
                    *state = State::Normal;
                    // Re-evaluate current byte `b` in State::Normal.
                    continue;
                }
            },
            State::InLineComment => {
                // Fast inner loop for single-line comment.
                while i < end {
                    let cb = buffer[i];
                    if cb == b'\n' {
                        *state = State::Normal;
                        i += 1;
                        break;
                    } else if cb == b'\r' {
                        // Preserve carriage return.
                    } else {
                        buffer[i] = b' ';
                    }
                    i += 1;
                }
                continue;
            }
            State::InBlockComment => {
                // Fast inner loop for multi-line comment.
                while i < end {
                    let cb = buffer[i];
                    if cb == b'*' {
                        buffer[i] = b' ';
                        *state = State::InBlockCommentSawStar;
                        i += 1;
                        break;
                    } else if cb == b'\n' || cb == b'\r' {
                        // Preserve line breaks for exact error line reporting.
                    } else {
                        buffer[i] = b' ';
                    }
                    i += 1;
                }
                continue;
            }
            State::InBlockCommentSawStar => {
                if b == b'/' {
                    // Multi-line comment ends!
                    buffer[i] = b' ';
                    *state = State::Normal;
                } else if b == b'*' {
                    buffer[i] = b' ';
                    // Remain in InBlockCommentSawStar.
                } else if b == b'\n' || b == b'\r' {
                    *state = State::InBlockComment;
                } else {
                    buffer[i] = b' ';
                    *state = State::InBlockComment;
                }
            }
        }
        i += 1;
    }
}
