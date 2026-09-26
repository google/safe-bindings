use crate::{invalid_argument_error, StatusOr, Value};

use serde_json::value::RawValue;

// The tests ParseNumbersAsStringsTest.MatchesParseNestingLimit* ensure this
// matches serde_json's limit.
const SERDE_JSON_RECURSION_LIMIT: u32 = 128;

/// Parses the given JSON data into a [Value], representing each number as a
/// [`serde_json::Value::String`] holding that number's verbatim source text.
///
/// The result does not round-trip: serializing it writes each number as a JSON
/// string, so the input `1` is serialized as `"1"`.
///
/// Numbers are validated for syntax but not for range, so values that `Parse`
/// rejects as out of range, such as `1e309`, are accepted here and preserved
/// verbatim. This function is more permissive than `Parse` about numbers.
#[crubit_annotate::cpp_name("ParseNumbersAsStrings")]
pub fn parse_numbers_as_strings(data: &[u8]) -> StatusOr<Value> {
    match parse(data) {
        Ok(value) => Ok(Value::new(value)),
        Err(message) => Err(invalid_argument_error(message)),
    }
}

/// Parses `data` in two passes: serde_json validates the whole document, then
/// a scanner walks the validated text to build the tree.
///
/// Returns an error message for invalid JSON. Panics if the scanner disagrees
/// with serde_json about the validated text, which would be a bug in the
/// scanner.
fn parse(data: &[u8]) -> Result<serde_json::Value, String> {
    // Pass 1: serde_json checks the whole document without building anything.
    let raw: &RawValue = serde_json::from_slice(data).map_err(|err| err.to_string())?;
    let text = raw.get();

    // Pass 2: walk the now-trusted text once, building the tree.
    let mut scanner = ValidatedScanner { text, bytes: text.as_bytes(), pos: 0 };
    let value = scanner.value(0)?;
    if scanner.pos != scanner.bytes.len() {
        scanner.unexpected_input();
    }
    Ok(value)
}

fn check_container_depth(depth: u32) -> Result<(), String> {
    if depth + 1 >= SERDE_JSON_RECURSION_LIMIT {
        return Err("recursion limit exceeded".to_owned());
    }
    Ok(())
}

struct ValidatedScanner<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl ValidatedScanner<'_> {
    fn value(&mut self, depth: u32) -> Result<serde_json::Value, String> {
        self.skip_whitespace();
        match self.peek() {
            b'{' => self.object(depth),
            b'[' => self.array(depth),
            b'"' => Ok(serde_json::Value::String(self.string()?)),
            b't' => Ok(self.literal("true", serde_json::Value::Bool(true))),
            b'f' => Ok(self.literal("false", serde_json::Value::Bool(false))),
            b'n' => Ok(self.literal("null", serde_json::Value::Null)),
            b'-' | b'0'..=b'9' => Ok(serde_json::Value::String(self.number().to_owned())),
            _ => self.unexpected_input(),
        }
    }

    fn object(&mut self, depth: u32) -> Result<serde_json::Value, String> {
        check_container_depth(depth)?;
        self.expect(b'{');
        let mut object = serde_json::Map::new();
        self.skip_whitespace();
        if self.peek() == b'}' {
            self.pos += 1;
            return Ok(serde_json::Value::Object(object));
        }
        loop {
            self.skip_whitespace();
            let key = self.string()?;
            self.skip_whitespace();
            self.expect(b':');
            let value = self.value(depth + 1)?;
            object.insert(key, value);
            self.skip_whitespace();
            match self.next() {
                b',' => {}
                b'}' => return Ok(serde_json::Value::Object(object)),
                _ => self.unexpected_input(),
            }
        }
    }

    fn array(&mut self, depth: u32) -> Result<serde_json::Value, String> {
        check_container_depth(depth)?;
        self.expect(b'[');
        let mut elements = Vec::new();
        self.skip_whitespace();
        if self.peek() == b']' {
            self.pos += 1;
            return Ok(serde_json::Value::Array(elements));
        }
        loop {
            elements.push(self.value(depth + 1)?);
            self.skip_whitespace();
            match self.next() {
                b',' => {}
                b']' => return Ok(serde_json::Value::Array(elements)),
                _ => self.unexpected_input(),
            }
        }
    }

    /// Returns the decoded string. Fails only for escapes that pass 1 accepts
    /// but that don't decode, such as a lone surrogate `"\ud800"`.
    fn string(&mut self) -> Result<String, String> {
        let start = self.pos;
        self.expect(b'"');
        let mut has_escape = false;
        loop {
            match self.next() {
                b'"' => break,
                b'\\' => {
                    has_escape = true;
                    self.pos += 1;
                }
                _ => {}
            }
        }
        if has_escape {
            serde_json::from_str(&self.text[start..self.pos]).map_err(|err| err.to_string())
        } else {
            Ok(self.text[start + 1..self.pos - 1].to_owned())
        }
    }

    fn number(&mut self) -> &str {
        let start = self.pos;
        while let Some(b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E') = self.bytes.get(self.pos) {
            self.pos += 1;
        }
        &self.text[start..self.pos]
    }

    fn literal(&mut self, spelling: &str, value: serde_json::Value) -> serde_json::Value {
        if !self.bytes[self.pos..].starts_with(spelling.as_bytes()) {
            self.unexpected_input();
        }
        self.pos += spelling.len();
        value
    }

    fn skip_whitespace(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.bytes.get(self.pos) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> u8 {
        match self.bytes.get(self.pos) {
            Some(&byte) => byte,
            None => self.unexpected_input(),
        }
    }

    fn next(&mut self) -> u8 {
        let byte = self.peek();
        self.pos += 1;
        byte
    }

    fn expect(&mut self, byte: u8) {
        if self.next() != byte {
            self.unexpected_input();
        }
    }

    /// Panics: the scanner only sees text serde_json already validated, so
    /// reaching this is a bug in the scanner, not bad input.
    #[cold]
    fn unexpected_input(&self) -> ! {
        panic!("unexpected input at byte {} of JSON validated by serde_json", self.pos)
    }
}
