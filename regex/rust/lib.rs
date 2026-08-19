//! This crate provides C++ bindings for the regex crate.
//!
//! It exposes the regex API in a way that works around the limitations of
//! crubit, so that C++ bindings can be automatically generated.
//!
//! WARNING: This crate should never be used from Rust. Use regex directly.

pub mod regex_rewrite;
mod vec_u8;

macro_rules! error { ($($arg:tt)*) => { eprintln!($($arg)*); }; }
use num_traits::Num;
use regex_automata::{meta, util::syntax, MatchKind};
use std::option::Option;
use std::sync::Arc;
pub use vec_u8::VecU8;

// Wrapper structs for structs in the regex package that we need to expose. We use `Option` for the
// inner object so that we can use `#[derive(Default)]` even when the inner types are not Default.
// This makes wrapper objects movable by Crubit, which is necessary for useful things like returning
// them by value.
//
// A problem with this approach is that it adds a new state to objects, which can now be empty.
// This will happen if the user calls a method on a moved-from object. For these objects, doing so
// is always a bug, and ideally we'd like to call `unwrap()` and let the program crash. However,
// this is not acceptable in production code for some projects, so we return a sensible default
// value and log an error instead.

/// An opaque representation of a match found in a haystack. 'h is the lifetime of the haystack.
#[derive(Clone, Default, Debug)]
pub struct Match<'h> {
    slice: &'h [u8],
    start: usize,
}

impl<'h> Match<'h> {
    pub fn new(haystack: &'h [u8], start: usize, end: usize) -> Self {
        Self { slice: &haystack[start..end], start }
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.start + self.slice.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slice.is_empty()
    }

    pub fn len(&self) -> usize {
        self.slice.len()
    }

    // NOTE(b/469976097): Decide if we want to support `range()` here. It can be implemented in the
    // C++ side with `start()` and `end()` anyway.

    pub fn as_str(&self) -> &'h [u8] {
        self.slice
    }

    pub fn as_bytes(&self) -> &'h [u8] {
        self.slice
    }

    pub fn parse_as_i8(&self, radix: i32) -> Result<i8, VecU8> {
        parse_integer(self.as_bytes(), radix).into()
    }
    pub fn parse_as_u8(&self, radix: i32) -> Result<u8, VecU8> {
        parse_integer(self.as_bytes(), radix).into()
    }
    pub fn parse_as_i16(&self, radix: i32) -> Result<i16, VecU8> {
        parse_integer(self.as_bytes(), radix).into()
    }
    pub fn parse_as_u16(&self, radix: i32) -> Result<u16, VecU8> {
        parse_integer(self.as_bytes(), radix).into()
    }
    pub fn parse_as_i32(&self, radix: i32) -> Result<i32, VecU8> {
        parse_integer(self.as_bytes(), radix).into()
    }
    pub fn parse_as_u32(&self, radix: i32) -> Result<u32, VecU8> {
        parse_integer(self.as_bytes(), radix).into()
    }
    pub fn parse_as_i64(&self, radix: i32) -> Result<i64, VecU8> {
        parse_integer(self.as_bytes(), radix).into()
    }
    pub fn parse_as_u64(&self, radix: i32) -> Result<u64, VecU8> {
        parse_integer(self.as_bytes(), radix).into()
    }
    pub fn parse_as_f32(&self) -> Result<f32, VecU8> {
        parse_float(self.as_bytes()).into()
    }
    pub fn parse_as_f64(&self) -> Result<f64, VecU8> {
        parse_float(self.as_bytes()).into()
    }
}

fn parse_integer<T>(slice: &[u8], mut radix: i32) -> Result<T, VecU8>
where
    T: Num,
    <T as Num>::FromStrRadixErr: std::fmt::Display,
{
    if slice.is_empty() {
        return Err("Empty match is not a valid integer".to_string().into());
    }

    // RE2 only parses ASCII numbers, and so do we.
    if !slice.is_ascii() {
        return Err("Non-ASCII match is not a valid integer".to_string().into());
    }

    // An ASCII string is always valid UTF-8.
    let string =
        std::str::from_utf8(slice).map_err::<VecU8, _>(|_| "Invalid Utf8".to_string().into())?;

    // RE2 doesn't allow leading spaces for integers.
    if string.starts_with(|c: char| c.is_whitespace()) {
        return Err("Leading whitespace not allowed for integers".to_string().into());
    }

    let original_string_with_sign = string;
    let mut string = string;
    let mut neg = false;
    if string.starts_with('-') {
        neg = true;
        string = &string[1..];
    } else if string.starts_with('+') {
        string = &string[1..];
    }

    let mut prefix_skipped = false;
    // Skip an arbitrary number of leading zeros. This allows us to use a small buffer
    // below while still supporting any valid string containing a number that fits in a u64.
    // We leave two zeros in place to avoid turning "000x123" (invalid) into "0x123" (valid).
    if string.starts_with("00") {
        let mut i = 2;
        while i < string.len() && string.as_bytes()[i] == b'0' {
            i += 1;
        }
        if i > 2 {
            string = &string[(i - 2)..];
            prefix_skipped = true;
        }
    }
    // Detect radix if not provided.
    if radix == 0 {
        if string.starts_with("0x") || string.starts_with("0X") {
            radix = 16;
            string = &string[2..];
            prefix_skipped = true;
        } else if string.starts_with('0') && string.len() > 1 {
            radix = 8;
            string = &string[1..];
            prefix_skipped = true;
        } else {
            radix = 10;
        }
    } else if radix == 16 && (string.starts_with("0x") || string.starts_with("0X")) {
        string = &string[2..];
        prefix_skipped = true;
    }

    if string.is_empty() {
        return Err("No digits found".to_string().into());
    }

    // When dealing with a negative number, we can't parse it as positive and then negate it,
    // because abs(T::MIN) does not fit in T. If we skipped a prefix, we need to reconstruct the
    // string with just the sign and value into a stack-allocated buffer. Otherwise we can just use
    // the original string.
    let val = if prefix_skipped {
        let mut buf = [0u8; 32]; // enough space for up to 64 bit integers.
        let mut len = 0;
        if neg {
            buf[len] = b'-';
            len += 1;
        }
        let string_bytes = string.as_bytes();
        if len + string_bytes.len() > buf.len() {
            return Err(
                format!("Can't parse {} as integer: too long", original_string_with_sign).into()
            );
        }
        buf[len..len + string_bytes.len()].copy_from_slice(string_bytes);
        len += string_bytes.len();
        let reconstructed_string = std::str::from_utf8(&buf[..len]).unwrap();
        T::from_str_radix(reconstructed_string, radix as u32)
    } else {
        T::from_str_radix(original_string_with_sign, radix as u32)
    };

    val.map_err(|e| {
        format!(
            "Error parsing {} as a {} in base {}: {}",
            original_string_with_sign,
            std::any::type_name::<T>(),
            radix,
            e
        )
        .into()
    })
}

fn parse_float<T>(slice: &[u8]) -> Result<T, VecU8>
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::error::Error + Send + Sync + 'static,
{
    // RE2 only parses ASCII numbers, and so do we.
    if !slice.is_ascii() {
        return Err("Non-ASCII match is not a valid float".to_string().into());
    }
    // An ASCII string is always valid UTF-8.
    let s =
        std::str::from_utf8(slice).map_err::<VecU8, _>(|_| "Invalid Utf8".to_string().into())?;
    // RE2 allows leading spaces for floats.
    s.trim_start().parse::<T>().map_err(|e| {
        format!("Error parsing {} as a {}: {}", s, std::any::type_name::<T>(), e).into()
    })
}

/// Opaque wrapper for `Matches`, an iterator over matches in a haystack.
/// 'r is the lifetime of the compiled regex, 'h is the lifetime of the haystack.
#[derive(Default, Debug)]
pub struct Matches<'r, 'h> {
    haystack: &'h [u8],
    inner: Option<meta::FindMatches<'r, 'h>>,
}

impl<'r, 'h> Matches<'r, 'h> {
    // NOTE(b/483382648): Make `Matches` implement `Iterator`.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<Match<'h>> {
        let haystack = self.haystack;
        self.inner.as_mut().map_or_else(
            || {
                error!("Use of moved-from Matches");
                None
            },
            |i| i.next().map(|m| Match::new(haystack, m.start(), m.end())),
        )
    }
}

/// Opaque wrapper for `Captures`. 'h is the lifetime of the haystack.
#[derive(Clone, Default, Debug)]
pub struct Captures<'h> {
    haystack: &'h [u8],
    inner: Option<regex_automata::util::captures::Captures>,
}

impl<'h> Captures<'h> {
    pub fn new(haystack: &'h [u8], caps: regex_automata::util::captures::Captures) -> Self {
        Self { haystack, inner: Some(caps) }
    }

    pub fn get(&self, i: usize) -> Option<Match<'h>> {
        let haystack = self.haystack;
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from Captures");
                None
            },
            |inner| inner.get_group(i).map(|span| Match::new(haystack, span.start, span.end)),
        )
    }

    pub fn get_match(&self) -> Match<'h> {
        let haystack = self.haystack;
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from Captures");
                Default::default()
            },
            |i| i.get_match().map(|m| Match::new(haystack, m.start(), m.end())).unwrap_or_default(),
        )
    }

    pub fn name(&self, name: &str) -> Option<Match<'h>> {
        let haystack = self.haystack;
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from Captures");
                None
            },
            |inner| {
                inner.get_group_by_name(name).map(|span| Match::new(haystack, span.start, span.end))
            },
        )
    }

    pub fn expand(&self, replacement: &[u8]) -> VecU8 {
        let Some(inner) = &self.inner else {
            error!("Use of moved-from Captures");
            return VecU8::from("");
        };
        let mut dst = Vec::<u8>::new();
        inner.interpolate_bytes_into(self.haystack, replacement, &mut dst);
        VecU8::from(dst)
    }

    // NOTE(b/259749023): implement `extract<N>` when crubit supports generic functions.

    pub fn iter<'c>(&'c self) -> SubCaptureMatches<'c, 'h> {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from Captures");
                Default::default()
            },
            |i| SubCaptureMatches { caps: Some(self), index: 0, len: i.group_len() },
        )
    }

    // `regex::Captures` doesn't have an `is_empty` method, so this wrapper doesn't either.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from Captures");
                0
            },
            |i| i.group_len(),
        )
    }
}

/// An opaque iterator over the capture groups in a single match. 'c is the lifetime of the `Captures` value, and 'h is
/// the lifetime of the haystack.
#[derive(Default, Debug)]
pub struct SubCaptureMatches<'c, 'h> {
    caps: Option<&'c Captures<'h>>,
    index: usize,
    len: usize,
}

impl<'c, 'h> SubCaptureMatches<'c, 'h> {
    // NOTE(b/483382648): Make `SubCaptureMatches` implement `Iterator`.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<Option<Match<'h>>> {
        if self.caps.is_none() {
            error!("Use of moved-from SubCaptureMatches");
            return None;
        }
        if self.index >= self.len {
            return None;
        }
        let caps = self.caps.unwrap();
        let res = caps.get(self.index);
        self.index += 1;
        Some(res)
    }
}

/// Opaque wrapper for `CaptureMatches`. 'r is the lifetime of the compiled regex, and 'h is the
/// lifetime of the haystack.
#[derive(Default, Debug)]
pub struct CaptureMatches<'r, 'h> {
    haystack: &'h [u8],
    inner: Option<meta::CapturesMatches<'r, 'h>>,
}

impl<'r, 'h> CaptureMatches<'r, 'h> {
    // NOTE(b/483382648): Make `CaptureMatches` implement `Iterator`.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<Captures<'h>> {
        let haystack = self.haystack;
        self.inner.as_mut().map_or_else(
            || {
                error!("Use of moved-from CaptureMatches");
                None
            },
            |i| i.next().map(|caps| Captures::new(haystack, caps)),
        )
    }
}

/// Opaque wrapper for `Split`. 'r is the lifetime of the compiled regular expression and 'h is the
/// lifetime of the haystack.
#[derive(Default, Debug)]
pub struct Split<'r, 'h> {
    haystack: &'h [u8],
    inner: Option<meta::Split<'r, 'h>>,
}

impl<'r, 'h> Split<'r, 'h> {
    // NOTE(b/483382648): Make `Split` implement `Iterator`.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&'h [u8]> {
        let haystack = self.haystack;
        self.inner.as_mut().map_or_else(
            || {
                error!("Use of moved-from Split");
                None
            },
            |i| i.next().map(|span| &haystack[span.start..span.end]),
        )
    }
}

/// Opaque wrapper for `SplitN`. 'r is the lifetime of the compiled regular expression and 'h is the
/// lifetime of the haystack.
#[derive(Default, Debug)]
pub struct SplitN<'r, 'h> {
    haystack: &'h [u8],
    inner: Option<meta::SplitN<'r, 'h>>,
}

impl<'r, 'h> SplitN<'r, 'h> {
    // NOTE(b/483382648): Make `SplitN` implement `Iterator`.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&'h [u8]> {
        let haystack = self.haystack;
        self.inner.as_mut().map_or_else(
            || {
                error!("Use of moved-from SplitN");
                None
            },
            |i| i.next().map(|span| &haystack[span.start..span.end]),
        )
    }
}

/// Opaque wrapper for `CaptureNames`, an iterator over the capture names in a regex.
/// 'r is the lifetime of the compiled regular expression.
#[derive(Default, Debug)]
pub struct CaptureNames<'r> {
    inner: Option<regex_automata::util::captures::GroupInfoPatternNames<'r>>,
}

impl<'r> CaptureNames<'r> {
    // NOTE(b/483382648): Make `CaptureNames` implement `Iterator`.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<Option<&'r [u8]>> {
        // The outer Option has value if there's another capture.
        // The inner Option has value if the capture has a name.
        self.inner.as_mut().map_or_else(
            || {
                error!("Use of moved-from CaptureNames");
                None
            },
            |i| i.next().map(|maybe_name| maybe_name.map(|val| val.as_bytes())),
        )
    }
}

/// Opaque wrapper for the result of a regex replacement operation, including
/// the number of replacements made and the resulting bytes.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct ReplaceResult {
    count: usize,
    result: VecU8,
}

impl ReplaceResult {
    pub fn new(count: usize, result: VecU8) -> Self {
        Self { count, result }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn result(&self) -> &VecU8 {
        &self.result
    }

    pub fn into_result(self) -> VecU8 {
        self.result
    }
}

/// Opaque wrapper for Regex object. We keep the inner regex in an Option to make this object
/// implement Default, and thus be movable.
#[derive(Clone, Default, Debug)]
pub struct Regex {
    inner: Option<meta::Regex>,
    pattern: Arc<str>,
}

impl Regex {
    // Disable the Clippy warning in order to follow the Regex API.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(val: &[u8]) -> Result<Regex, VecU8> {
        let builder = RegexBuilder::new(val);
        builder.build()
    }

    pub fn as_str(&self) -> &str {
        &self.pattern
    }

    pub fn is_match(&self, haystack: &[u8]) -> bool {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from Regex");
                false
            },
            |i| i.is_match(haystack),
        )
    }

    pub fn find<'h>(&self, haystack: &'h [u8]) -> Option<Match<'h>> {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from Regex");
                None
            },
            |i| i.find(haystack).map(|m| Match::new(haystack, m.start(), m.end())),
        )
    }

    pub fn find_iter<'r, 'h>(&'r self, haystack: &'h [u8]) -> Matches<'r, 'h> {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from Regex");
                Default::default()
            },
            |i| Matches { haystack, inner: Some(i.find_iter(haystack)) },
        )
    }

    pub fn captures<'h>(&self, haystack: &'h [u8]) -> Option<Captures<'h>> {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from Regex");
                None
            },
            |i| {
                let mut caps = i.create_captures();
                i.captures(haystack, &mut caps);
                if caps.is_match() {
                    Some(Captures::new(haystack, caps))
                } else {
                    None
                }
            },
        )
    }

    pub fn captures_iter<'r, 'h>(&'r self, haystack: &'h [u8]) -> CaptureMatches<'r, 'h> {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from Regex");
                Default::default()
            },
            |i| CaptureMatches { haystack, inner: Some(i.captures_iter(haystack)) },
        )
    }

    pub fn captures_len(&self) -> usize {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from Regex");
                0
            },
            |i| i.captures_len(),
        )
    }

    pub fn capture_names(&self) -> CaptureNames {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from Regex");
                Default::default()
            },
            |i| CaptureNames {
                inner: Some(i.group_info().pattern_names(regex_automata::PatternID::ZERO)),
            },
        )
    }

    // NOTE(b/393069993): The Rust version of the `replace*` methods return a `Cow` that borrows the
    // original haystack if no replacements are made, or a new string otherwise. Crubit doesn't
    // support `Cow`, so we always allocate a new string here for simplicity.
    // NOTE(b/469976097): The original `replace*` methods support passing a function to perform
    // replacements. Decide if we want to support this.
    pub fn replace(&self, haystack: &[u8], rep: &[u8]) -> VecU8 {
        self.replacen(haystack, 1, rep).into_result()
    }

    pub fn replace_all(&self, haystack: &[u8], rep: &[u8]) -> VecU8 {
        self.replacen(haystack, usize::MAX, rep).into_result()
    }

    pub fn replacen(&self, haystack: &[u8], limit: usize, rep: &[u8]) -> ReplaceResult {
        let Some(re) = &self.inner else {
            error!("Use of moved-from Regex");
            return ReplaceResult::default();
        };
        let limit = if limit == 0 { usize::MAX } else { limit };
        let mut it = re.captures_iter(haystack);
        let mut new = Vec::with_capacity(haystack.len());
        let mut last_match = 0;
        let mut count = 0;
        while count < limit {
            if let Some(caps) = it.next() {
                if let Some(m) = caps.get_match() {
                    new.extend_from_slice(&haystack[last_match..m.start()]);
                    caps.interpolate_bytes_into(haystack, rep, &mut new);
                    last_match = m.end();
                    count += 1;
                }
            } else {
                break;
            }
        }
        new.extend_from_slice(&haystack[last_match..]);
        ReplaceResult { count, result: VecU8::from(new) }
    }

    pub fn split<'r, 'h>(&'r self, haystack: &'h [u8]) -> Split<'r, 'h> {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from Regex");
                Default::default()
            },
            |i| Split { haystack, inner: Some(i.split(haystack)) },
        )
    }

    pub fn splitn<'r, 'h>(&'r self, haystack: &'h [u8], limit: usize) -> SplitN<'r, 'h> {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from Regex");
                Default::default()
            },
            |i| SplitN { haystack, inner: Some(i.splitn(haystack, limit)) },
        )
    }
}

// Default size limits matching the standard defaults of the upstream `regex` and `regex-automata`
// crates (from github.com/rust-lang/regex):
// - `DEFAULT_NFA_SIZE_LIMIT` (10 MiB): Matches `regex::RegexBuilder::size_limit` and
//   `regex_automata::meta::Config::nfa_size_limit`, preventing memory explosion during Thompson NFA
//   construction for complex patterns.
// - `DEFAULT_HYBRID_CACHE_CAPACITY` (2 MiB): Matches `regex::RegexBuilder::dfa_size_limit` and
//   `regex_automata::meta::Config::hybrid_cache_capacity`, setting the capacity for the lazy Hybrid
//   DFA transition cache.
const DEFAULT_NFA_SIZE_LIMIT: usize = 10 * (1 << 20);
const DEFAULT_HYBRID_CACHE_CAPACITY: usize = 2 * (1 << 20);

/// An opaque builder for configuring and compiling a `Regex`.
///
/// In this case, we DO use the fact that `pattern` can be none. We need to create a `str` from the
/// given pattern which, coming from C++, may contain invalid UTF-8. When it happens, we set
/// `pattern` to `None`, turn all the setters into no-ops, and return a special error for this case
/// from `build()`.
#[derive(Clone, Debug)]
pub struct RegexBuilder {
    pattern: Option<String>,
    metac: meta::Config,
    syntaxc: syntax::Config,
}

impl Default for RegexBuilder {
    fn default() -> Self {
        Self {
            pattern: None,
            metac: meta::Config::new()
                // Standard leftmost-first match semantics (same as RE2 / PCRE).
                .match_kind(MatchKind::LeftmostFirst)
                // Allow empty matches on any byte offset (supports arbitrary byte haystacks).
                .utf8_empty(false)
                // Cap NFA memory to prevent resource exhaustion on pathological patterns.
                .nfa_size_limit(Some(DEFAULT_NFA_SIZE_LIMIT))
                // Lazy Hybrid DFA cache capacity (2 MiB).
                .hybrid_cache_capacity(DEFAULT_HYBRID_CACHE_CAPACITY)
                // Disable fully ahead-of-time (AOT) dense DFA compilation to keep regex
                // compilation fast and lightweight; regex-automata uses the lazy Hybrid DFA
                // and PikeVM instead.
                .dfa(false),
            // Parse in byte-oriented mode to support raw byte slices.
            syntaxc: syntax::Config::new().utf8(false),
        }
    }
}

impl RegexBuilder {
    pub fn new(pattern: &[u8]) -> Self {
        let s = std::str::from_utf8(pattern);
        RegexBuilder {
            pattern: s.ok().map(|str_slice| str_slice.to_string()),
            ..Default::default()
        }
    }

    pub fn build(&self) -> Result<Regex, VecU8> {
        if let Some(pattern) = &self.pattern {
            let metac = self.metac.clone().match_kind(MatchKind::LeftmostFirst).utf8_empty(false);
            let syntaxc = self.syntaxc.utf8(false);
            let meta = meta::Builder::new()
                .configure(metac)
                .syntax(syntaxc)
                .build(pattern)
                .map_err::<VecU8, _>(|err| err.to_string().into())?;
            Ok(Regex { inner: Some(meta), pattern: Arc::from(pattern.as_str()) })
        } else {
            Err("Invalid UTF-8 in pattern".to_string().into())
        }
    }

    pub fn unicode(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.unicode(yes);
    }

    pub fn case_insensitive(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.case_insensitive(yes);
    }

    pub fn multi_line(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.multi_line(yes);
    }

    pub fn dot_matches_new_line(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.dot_matches_new_line(yes);
    }

    pub fn crlf(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.crlf(yes);
    }

    pub fn line_terminator(&mut self, byte: u8) {
        self.metac = self.metac.clone().line_terminator(byte);
        self.syntaxc = self.syntaxc.line_terminator(byte);
    }

    pub fn swap_greed(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.swap_greed(yes);
    }

    pub fn ignore_whitespace(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.ignore_whitespace(yes);
    }

    pub fn octal(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.octal(yes);
    }

    pub fn size_limit(&mut self, bytes: usize) {
        self.metac = self.metac.clone().nfa_size_limit(Some(bytes));
    }

    pub fn dfa_size_limit(&mut self, bytes: usize) {
        self.metac = self.metac.clone().hybrid_cache_capacity(bytes);
    }

    pub fn nest_limit(&mut self, limit: u32) {
        self.syntaxc = self.syntaxc.nest_limit(limit);
    }
}

/// Opaque wrapper for `SetMatches`.
#[derive(Default, Debug)]
pub struct SetMatches {
    inner: Option<regex_automata::PatternSet>,
}

impl SetMatches {
    pub fn matched(&self, regex_index: usize) -> bool {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from SetMatches");
                false
            },
            |i| {
                if let Ok(pid) = regex_automata::PatternID::new(regex_index) {
                    i.contains(pid)
                } else {
                    false
                }
            },
        )
    }

    /// Returns the number of regexes in the set, NOT the number of matches.
    /// You can find the indices of the matches by iterating all the expressions and
    /// calling `matched(i)`.
    pub fn len(&self) -> usize {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from SetMatches");
                0
            },
            |i| i.capacity(),
        )
    }

    /// Returns true only if there are no regular expressions in the set, independently of how many
    /// matches were found.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Opaque wrapper for RegexSet.
#[derive(Default, Debug)]
pub struct RegexSet {
    inner: Option<meta::Regex>,
}

impl RegexSet {
    /// Creates a new regex set from the given patterns. If any of the patterns fails to compile,
    /// it returns an error.
    #[allow(clippy::new_ret_no_self)] // We need to return a Result because compilation can fail.
    pub fn new(patterns: &[&[u8]]) -> Result<RegexSet, VecU8> {
        let builder = RegexSetBuilder::new(patterns);
        builder.build()
    }

    /// Returns true iff at least one of the regexes matches the haystack.
    pub fn is_match(&self, haystack: &[u8]) -> bool {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from RegexSet");
                false
            },
            |i| i.is_match(haystack),
        )
    }

    /// Returns a SetMatches object with information about which matches (if any) match the
    /// haystack.
    pub fn matches(&self, haystack: &[u8]) -> SetMatches {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from RegexSet");
                Default::default()
            },
            |i| {
                let mut pset = regex_automata::PatternSet::new(i.pattern_len());
                let input = regex_automata::Input::new(haystack);
                i.which_overlapping_matches(&input, &mut pset);
                SetMatches { inner: Some(pset) }
            },
        )
    }

    /// Returns the number of regexes in the set.
    pub fn len(&self) -> usize {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from RegexSet");
                0
            },
            |i| i.pattern_len(),
        )
    }

    /// Returns true iff there are no regular expressions in the set.
    pub fn is_empty(&self) -> bool {
        self.inner.as_ref().map_or_else(
            || {
                error!("Use of moved-from RegexSet");
                false
            },
            |i| i.pattern_len() == 0,
        )
    }
}

/// An opaque builder for configuring and compiling a `RegexSet`.
#[derive(Clone, Debug)]
pub struct RegexSetBuilder {
    patterns: Option<Vec<String>>,
    metac: meta::Config,
    syntaxc: syntax::Config,
}

impl Default for RegexSetBuilder {
    fn default() -> Self {
        Self {
            patterns: None,
            metac: meta::Config::new()
                // Report all matching pattern IDs in the set rather than stopping at the first.
                .match_kind(MatchKind::All)
                // Allow empty matches on any byte offset (supports arbitrary byte haystacks).
                .utf8_empty(false)
                // Disable capture group tracking since RegexSet only checks set membership.
                .which_captures(regex_automata::nfa::thompson::WhichCaptures::None)
                // Cap NFA memory to prevent resource exhaustion on pathological patterns.
                .nfa_size_limit(Some(DEFAULT_NFA_SIZE_LIMIT))
                // Lazy Hybrid DFA cache capacity (2 MiB).
                .hybrid_cache_capacity(DEFAULT_HYBRID_CACHE_CAPACITY)
                // Disable fully ahead-of-time (AOT) dense DFA compilation.
                .dfa(false),
            // Parse in byte-oriented mode to support raw byte slices.
            syntaxc: syntax::Config::new().utf8(false),
        }
    }
}

impl RegexSetBuilder {
    pub fn new(patterns: &[&[u8]]) -> Self {
        let mut exprs = Vec::with_capacity(patterns.len());
        let mut ok = true;
        for p in patterns {
            if let Ok(s) = std::str::from_utf8(p) {
                exprs.push(s.to_string());
            } else {
                ok = false;
                break;
            }
        }
        RegexSetBuilder { patterns: if ok { Some(exprs) } else { None }, ..Default::default() }
    }

    pub fn build(&self) -> Result<RegexSet, VecU8> {
        if let Some(patterns) = &self.patterns {
            let metac = self
                .metac
                .clone()
                .match_kind(MatchKind::All)
                .utf8_empty(false)
                .which_captures(regex_automata::nfa::thompson::WhichCaptures::None);
            let syntaxc = self.syntaxc.utf8(false);
            let meta = meta::Builder::new()
                .configure(metac)
                .syntax(syntaxc)
                .build_many(patterns)
                .map_err::<VecU8, _>(|err| err.to_string().into())?;
            Ok(RegexSet { inner: Some(meta) })
        } else {
            Err("Invalid UTF-8 in pattern".to_string().into())
        }
    }

    pub fn unicode(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.unicode(yes);
    }

    pub fn case_insensitive(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.case_insensitive(yes);
    }

    pub fn multi_line(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.multi_line(yes);
    }

    pub fn dot_matches_new_line(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.dot_matches_new_line(yes);
    }

    pub fn crlf(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.crlf(yes);
    }

    pub fn line_terminator(&mut self, byte: u8) {
        self.metac = self.metac.clone().line_terminator(byte);
        self.syntaxc = self.syntaxc.line_terminator(byte);
    }

    pub fn swap_greed(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.swap_greed(yes);
    }

    pub fn ignore_whitespace(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.ignore_whitespace(yes);
    }

    pub fn octal(&mut self, yes: bool) {
        self.syntaxc = self.syntaxc.octal(yes);
    }

    pub fn size_limit(&mut self, bytes: usize) {
        self.metac = self.metac.clone().nfa_size_limit(Some(bytes));
    }

    pub fn dfa_size_limit(&mut self, bytes: usize) {
        self.metac = self.metac.clone().hybrid_cache_capacity(bytes);
    }

    pub fn nest_limit(&mut self, limit: u32) {
        self.syntaxc = self.syntaxc.nest_limit(limit);
    }
}

#[cfg(test)]
mod tests {
        use super::*;
    use googletest::prelude::*;

    macro_rules! check_parse_integer {
        ($type:ident, $radix:expr, $in:expr, $out: expr) => {
            expect_that!(parse_integer::<$type>($in.as_bytes(), $radix), ok(eq(&$out)));
        };
    }

    macro_rules! check_bad_integer {
        ($type:ident, $radix:expr, $in:expr, $err: expr) => {
            expect_that!(
                parse_integer::<$type>($in.as_bytes(), $radix),
                err(displays_as(contains_substring($err)))
            );
        };
    }

    #[gtest]
    fn test_hex() {
        // Check hex parsing when radix=16.
        check_parse_integer!(i16, 16, "2bad", 0x2bad);
        check_parse_integer!(u16, 16, "2bad", 0x2bad);
        check_parse_integer!(i32, 16, "dead", 0xdead);
        check_parse_integer!(u32, 16, "dead", 0xdead);
        check_parse_integer!(i32, 16, "7eadbeef", 0x7eadbeef);
        check_parse_integer!(u32, 16, "deadbeef", 0xdeadbeef);
        check_parse_integer!(i64, 16, "12345678deadbeef", 0x12345678deadbeef);
        check_parse_integer!(u64, 16, "cafebabedeadbeef", 0xcafebabedeadbeef);
        // Check hex parsing when detecting radix (radix=0).
        check_parse_integer!(i16, 0, "0x2bad", 0x2bad);
        check_parse_integer!(u16, 0, "0x2bad", 0x2bad);
        check_parse_integer!(i32, 0, "0xdead", 0xdead);
        check_parse_integer!(u32, 0, "0xdead", 0xdead);
        check_parse_integer!(i32, 0, "0x7eadbeef", 0x7eadbeef);
        check_parse_integer!(u32, 0, "0xdeadbeef", 0xdeadbeef);
        check_parse_integer!(i64, 0, "0x12345678deadbeef", 0x12345678deadbeef);
        check_parse_integer!(u64, 0, "0xcafebabedeadbeef", 0xcafebabedeadbeef);
    }

    #[gtest]
    fn test_octal() {
        // Check octal parsing when radix=8.
        check_parse_integer!(i16, 8, "77777", 0o77777);
        check_parse_integer!(u16, 8, "177777", 0o177777);
        check_parse_integer!(i32, 8, "17777777777", 0o17777777777);
        check_parse_integer!(u32, 8, "37777777777", 0o37777777777);
        check_parse_integer!(i64, 8, "777777777777777777777", 0o777777777777777777777);
        check_parse_integer!(u64, 8, "1777777777777777777777", 0o1777777777777777777777);
        // Check octal parsing when detecting radix (radix=0).
        check_parse_integer!(i16, 0, "077777", 0o77777);
        check_parse_integer!(u16, 0, "0177777", 0o177777);
        check_parse_integer!(i32, 0, "017777777777", 0o17777777777);
        check_parse_integer!(u32, 0, "037777777777", 0o37777777777);
        check_parse_integer!(i64, 0, "0777777777777777777777", 0o777777777777777777777);
        check_parse_integer!(u64, 0, "01777777777777777777777", 0o1777777777777777777777);
    }

    #[gtest]
    fn test_decimal() {
        // Check decimal parsing when radix=10.
        check_parse_integer!(i16, 10, "-1", -1);
        check_parse_integer!(u16, 10, "9999", 9999);
        check_parse_integer!(i32, 10, "-1000", -1000);
        check_parse_integer!(u32, 10, "12345", 12345);
        check_parse_integer!(i32, 10, "-10000000", -10000000);
        check_parse_integer!(u32, 10, "3083324652", 3083324652);
        check_parse_integer!(i64, 10, "-100000000000000", -100000000000000);
        check_parse_integer!(u64, 10, "1234567890987654321", 1234567890987654321);
        // Check decimal parsing when detecting radix (radix=10).
        check_parse_integer!(i16, 0, "-1", -1);
        check_parse_integer!(u16, 0, "9999", 9999);
        check_parse_integer!(i32, 0, "-1000", -1000);
        check_parse_integer!(u32, 0, "12345", 12345);
        check_parse_integer!(i32, 0, "-10000000", -10000000);
        check_parse_integer!(u32, 0, "3083324652", 3083324652);
        check_parse_integer!(i64, 0, "-100000000000000", -100000000000000);
        check_parse_integer!(u64, 0, "1234567890987654321", 1234567890987654321);
    }

    #[gtest]
    fn test_i16() {
        check_parse_integer!(i16, 10, "100", 100);
        check_parse_integer!(i16, 10, "-100", -100);
        check_parse_integer!(i16, 10, "32767", 32767);
        check_parse_integer!(i16, 10, "-32768", -32768);
        check_bad_integer!(i16, 10, "-32769", "too small");
        check_bad_integer!(i16, 10, "32768", "too large");
    }

    #[gtest]
    fn test_u16() {
        check_parse_integer!(u16, 10, "100", 100);
        check_parse_integer!(u16, 10, "32767", 32767);
        check_parse_integer!(u16, 10, "65535", 65535);
        check_bad_integer!(u16, 10, "65536", "too large");
    }

    #[gtest]
    fn test_i32() {
        check_parse_integer!(i32, 10, "100", 100);
        check_parse_integer!(i32, 10, "-100", -100);
        check_parse_integer!(i32, 10, "2147483647", i32::MAX);
        check_parse_integer!(i32, 10, "-2147483648", i32::MIN);
        check_bad_integer!(i32, 10, "-2147483649", "too small");
        check_bad_integer!(i32, 10, "2147483648", "too large");

        let zeros = "0".repeat(1000);
        check_parse_integer!(i32, 10, &zeros, 0);
        check_parse_integer!(i32, 10, &format!("-{}", zeros), 0);
        check_parse_integer!(i32, 10, &format!("{}2147483647", zeros), i32::MAX);
        check_parse_integer!(i32, 10, &format!("-{}2147483648", zeros), i32::MIN);
        check_bad_integer!(i32, 10, &format!("{}2147483648", zeros), "too large");
        check_bad_integer!(i32, 10, &format!("-{}2147483649", zeros), "too small");

        check_parse_integer!(i32, 0, "0x7fffffff", i32::MAX);
        check_parse_integer!(i32, 0, "-0x80000000", i32::MIN);
        check_bad_integer!(i32, 0, "0x80000000", "too large");
        check_bad_integer!(i32, 0, "-0x80000001", "too small");

        check_bad_integer!(i32, 0, "𝟙𝟘", "Non-ASCII");

        // We could in theory support these, the same way we support the decimal ones.
        // But RE2 doesn't trim leading zeros after the 0x prefix, so we don't either.
        check_bad_integer!(i32, 0, &format!("0x{}7fffffff", zeros), "too long");
        check_bad_integer!(i32, 0, &format!("-0x{}80000000", zeros), "too long");

        // 000x gets detected as octal, then fails.
        check_bad_integer!(i32, 0, "000x7fffffff", "invalid digit");
    }

    #[gtest]
    fn test_u32() {
        check_parse_integer!(u32, 10, "100", 100);
        check_parse_integer!(u32, 10, "4294967295", u32::MAX);
        check_bad_integer!(u32, 10, "4294967296", "too large");
        check_bad_integer!(u32, 10, "-1", "invalid digit");

        let zeros = "0".repeat(1000);
        check_parse_integer!(u32, 10, &format!("{}4294967295", zeros), u32::MAX);
    }

    #[gtest]
    fn test_i64() {
        check_parse_integer!(i64, 10, "100", 100);
        check_parse_integer!(i64, 10, "-100", -100);
        check_parse_integer!(i64, 10, "9223372036854775807", i64::MAX);
        check_parse_integer!(i64, 10, "-9223372036854775808", i64::MIN);
        check_bad_integer!(i64, 10, "-9223372036854775809", "too small");
        check_bad_integer!(i64, 10, "9223372036854775808", "too large");
    }

    #[gtest]
    fn test_u64() {
        check_parse_integer!(u64, 10, "100", 100);
        check_parse_integer!(u64, 10, "18446744073709551615", u64::MAX);
        check_bad_integer!(u64, 10, "18446744073709551616", "too large");
        check_bad_integer!(u64, 10, "-1", "invalid digit");
    }

    macro_rules! check_parse_float {
        ($type:ident, $in:expr, $out: expr) => {
            expect_that!(parse_float::<$type>($in.as_bytes()), ok(eq(&$out)));
        };
    }

    macro_rules! check_bad_float {
        ($type:ident, $in:expr, $err: expr) => {
            expect_that!(
                parse_float::<$type>($in.as_bytes()),
                err(displays_as(contains_substring($err)))
            );
        };
    }

    #[gtest]
    fn test_f32() {
        let zeros = "0".repeat(1000);
        check_parse_float!(f32, "100", 100.0f32);
        check_parse_float!(f32, "-100.", -100.0f32);
        check_parse_float!(f32, "1e23", 1e23f32);
        check_parse_float!(f32, "0.1", 0.1f32);
        check_parse_float!(f32, "1e-1", 0.1f32);
        check_parse_float!(f32, " 100", 100.0f32);
        check_parse_float!(f32, format!("{}1e23", zeros), 1e23f32);

        // 6700000000081920.1 is an edge case that will parse correctly with strtof but not with
        // strtod and a conversion to float. For a more complete explanation see
        // http://google3/third_party/re2/testing/re2_test.cc;l=911;rcl=784637099
        check_parse_float!(f32, "6700000000081920.1", 6700000000081920.1f32);

        // RE2 tests don't check these, but it's good to document this behavior.
        check_parse_float!(f32, "1e999", f32::INFINITY);
        check_parse_float!(f32, "-1e999", f32::NEG_INFINITY);
        check_parse_float!(f32, "1e-999", 0.0);
        check_parse_float!(f32, "-1e-999", -0.0);

        // Leading zeros in exponent.
        check_parse_float!(f32, "1e0002", 100.0f32);
        // RE2 doesn't actually allow floats to be this long, because it copies
        // them to a fixed-size buffer before calling `strtof` (see
        // http://google3/third_party/re2/re2.cc;l=1204;rcl=784637099).
        // But there's probably no harm in us being slightly more flexible here.
        check_parse_float!(f32, format!("1e{}2", zeros), 100.0f32);
        check_parse_float!(f32, format!("   1e{}2", zeros), 100.0f32);

        check_bad_float!(f32, "1.0.0", "invalid float");
        check_bad_float!(f32, "𝟙.𝟘", "Non-ASCII");
    }

    #[gtest]
    fn test_f64() {
        let zeros = "0".repeat(1000);
        check_parse_float!(f64, "100", 100.0f64);
        check_parse_float!(f64, "-100.", -100.0f64);
        check_parse_float!(f64, "1e23", 1e23f64);
        check_parse_float!(f64, "0.1", 0.1f64);
        check_parse_float!(f64, "1e-1", 0.1f64);
        check_parse_float!(f64, " 100", 100.0f64);
        check_parse_float!(f64, format!("{}1e23", zeros), 1e23f64);

        check_parse_float!(f64, "1.00000005960464485", 1.0000000596046448f64);

        // RE2 tests don't check these, but it's good to document this behavior.
        check_parse_float!(f64, "1e999", f64::INFINITY);
        check_parse_float!(f64, "-1e999", f64::NEG_INFINITY);
        check_parse_float!(f64, "1e-999", 0.0);
        check_parse_float!(f64, "-1e-999", -0.0);

        check_bad_float!(f64, "1.0.0", "invalid float");
    }

    #[gtest]
    fn test_regex_builder_line_terminator() {
        let mut builder = RegexBuilder::new(b".");
        builder.line_terminator(b'z');
        let regex = builder.build().unwrap();
        expect_that!(regex.is_match(b"\n"), eq(true));
        expect_that!(regex.is_match(b"z"), eq(false));

        let mut builder_multiline = RegexBuilder::new(b"^abc$");
        builder_multiline.multi_line(true);
        builder_multiline.line_terminator(b'z');
        let regex_multiline = builder_multiline.build().unwrap();
        expect_that!(regex_multiline.is_match(b"zabc"), eq(true));
        expect_that!(regex_multiline.is_match(b"abcz"), eq(true));
        expect_that!(regex_multiline.is_match(b"\nabc"), eq(false));
    }

    #[gtest]
    fn test_regex_set_builder_line_terminator() {
        let mut builder = RegexSetBuilder::new(&[b"."]);
        builder.line_terminator(b'z');
        let set = builder.build().unwrap();
        expect_that!(set.is_match(b"\n"), eq(true));
        expect_that!(set.is_match(b"z"), eq(false));

        let mut builder_multiline = RegexSetBuilder::new(&[b"^abc$"]);
        builder_multiline.multi_line(true);
        builder_multiline.line_terminator(b'z');
        let set_multiline = builder_multiline.build().unwrap();
        expect_that!(set_multiline.is_match(b"zabc"), eq(true));
        expect_that!(set_multiline.is_match(b"abcz"), eq(true));
        expect_that!(set_multiline.is_match(b"\nabc"), eq(false));
    }

    #[gtest]
    fn test_regex_builder_options() {
        // unicode
        let mut builder = RegexBuilder::new(b"\\w+");
        builder.unicode(false);
        let r = builder.build().unwrap();
        expect_that!(r.is_match("é".as_bytes()), eq(false));
        expect_that!(r.is_match(b"abc"), eq(true));

        // crlf
        let mut builder = RegexBuilder::new(b"^abc$");
        builder.multi_line(true);
        builder.crlf(true);
        let r = builder.build().unwrap();
        expect_that!(r.is_match(b"abc\r\n"), eq(true));

        // swap_greed
        let mut builder = RegexBuilder::new(b"a*");
        builder.swap_greed(true);
        let r = builder.build().unwrap();
        expect_that!(r.find(b"aaa").unwrap().len(), eq(0));

        // octal
        let mut builder = RegexBuilder::new(b"\\101");
        builder.octal(true);
        let r = builder.build().unwrap();
        expect_that!(r.is_match(b"A"), eq(true));

        // nest_limit
        let mut builder = RegexBuilder::new(b"ab");
        builder.nest_limit(0);
        expect_that!(builder.build().is_err(), eq(true));
    }

    #[gtest]
    fn test_regex_set_builder_options() {
        // crlf
        let mut builder = RegexSetBuilder::new(&[b"^abc$"]);
        builder.multi_line(true);
        builder.crlf(true);
        let set = builder.build().unwrap();
        expect_that!(set.is_match(b"abc\r\n"), eq(true));

        // octal
        let mut builder = RegexSetBuilder::new(&[b"\\101"]);
        builder.octal(true);
        let set = builder.build().unwrap();
        expect_that!(set.is_match(b"A"), eq(true));

        // nest_limit
        let mut builder = RegexSetBuilder::new(&[b"ab"]);
        builder.nest_limit(0);
        expect_that!(builder.build().is_err(), eq(true));
    }
}
