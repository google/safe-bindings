use crate::{invalid_argument_error, json_sanitizer, StatusOr, Value};

/// Parses the given JSON data into a [Value], ignoring comments.
///
/// Filters out:
/// 1. **Single-line comments**: `// ...` up to a newline or EOF.
/// 2. **Multi-line comments**: `/* ... */`.
/// 3. **Trailing commas**: Commas in arrays (`[...]`) or objects (`{...}`) that
///    are followed only by whitespace and/or comments before the closing bracket
///    or brace.
#[crubit_annotate::cpp_name("ParseIgnoreComments")]
pub fn parse_ignore_comments(data: &[u8]) -> StatusOr<Value> {
    let sanitizer = json_sanitizer::JsonSanitizer::new(data);
    match serde_json::from_reader(sanitizer) {
        Ok(value) => Ok(Value::new(value)),
        Err(err) => Err(invalid_argument_error(err.to_string())),
    }
}
