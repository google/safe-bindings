use crate::vec_u8::VecU8;
use saphyr::{LoadableYamlNode, ScalarOwned, YamlOwned};

/// Converts a YAML byte slice into a JSON string encoded as `VecU8`.
///
/// # Arguments
/// * `input` - Raw UTF-8 encoded YAML input bytes.
/// * `recursion_depth_limit` - Maximum allowable YAML node hierarchy depth.
///
/// # Errors
/// Returns an error `VecU8` if the input is invalid UTF-8, malformed YAML,
/// contains multiple YAML documents, or exceeds `recursion_depth_limit`.
pub fn convert_yaml_to_json(input: &[u8], recursion_depth_limit: i32) -> Result<VecU8, VecU8> {
    let input_str = match std::str::from_utf8(input) {
        Ok(s) => s,
        Err(_) => return Err(VecU8::from("Input is not valid UTF-8")),
    };
    let docs = match YamlOwned::load_from_str(input_str) {
        Ok(docs) => docs,
        Err(err) => return Err(VecU8::from(err.to_string())),
    };
    if docs.is_empty() {
        return Err(VecU8::from("Undefined YAML Node Type"));
    }
    if docs.len() > 1 {
        return Err(VecU8::from("Multiple YAML documents are not supported"));
    }
    let mut out = String::with_capacity(input.len() * 2);
    append_json_from_yaml_node(&docs[0], &mut out, 1, recursion_depth_limit)?;
    Ok(VecU8::from(out))
}

fn append_json_escaped_string(s: &str, out: &mut String) {
    use std::fmt::Write as _;
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => {
                if (c as u32) >= 0x20 {
                    out.push(c);
                } else {
                    let _ = write!(out, "\\u{:04x}", c as u32);
                }
            }
        }
    }
    out.push('"');
}

fn append_json_from_yaml_node(
    node: &YamlOwned,
    out: &mut String,
    depth: i32,
    limit: i32,
) -> Result<(), VecU8> {
    if depth > limit {
        return Err(VecU8::from(format!(
            "YAML node hierarchy is too deep to parse, depth limit: {}",
            limit
        )));
    }
    match node {
        YamlOwned::Value(scalar) => match scalar {
            ScalarOwned::Null => {
                out.push_str("null");
            }
            ScalarOwned::Boolean(val) => {
                out.push_str(if *val { "true" } else { "false" });
            }
            ScalarOwned::Integer(val) => {
                const MAX_SAFE_DOUBLE_INT: i64 = 9007199254740991;
                if *val >= -MAX_SAFE_DOUBLE_INT && *val <= MAX_SAFE_DOUBLE_INT {
                    out.push_str(&val.to_string());
                } else {
                    out.push('"');
                    out.push_str(&val.to_string());
                    out.push('"');
                }
            }
            ScalarOwned::FloatingPoint(val) => {
                if val.0.is_nan() {
                    out.push_str("\"NaN\"");
                } else if val.0 == f64::INFINITY {
                    out.push_str("\"Infinity\"");
                } else if val.0 == f64::NEG_INFINITY {
                    out.push_str("\"-Infinity\"");
                } else {
                    out.push_str(&val.0.to_string());
                }
            }
            ScalarOwned::String(val) => {
                append_json_escaped_string(val, out);
            }
        },
        YamlOwned::Sequence(seq) => {
            out.push('[');
            for (i, elem) in seq.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                append_json_from_yaml_node(elem, out, depth + 1, limit)?;
            }
            out.push(']');
        }
        YamlOwned::Mapping(map) => {
            out.push('{');
            for (i, (key, val)) in map.iter().enumerate() {
                let key_str = match key.as_str() {
                    Some(s) => s,
                    None => return Err(VecU8::from("YAML map key must be a string")),
                };
                if i > 0 {
                    out.push(',');
                }
                append_json_escaped_string(key_str, out);
                out.push(':');
                append_json_from_yaml_node(val, out, depth + 1, limit)?;
            }
            out.push('}');
        }
        YamlOwned::Tagged(_, inner) => {
            append_json_from_yaml_node(inner, out, depth, limit)?;
        }
        YamlOwned::Alias(_) | YamlOwned::Representation(..) | YamlOwned::BadValue => {
            return Err(VecU8::from("Undefined YAML Node Type"));
        }
    }
    Ok(())
}
