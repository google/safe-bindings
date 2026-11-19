#[macro_use]
mod macros;
mod iterator;
mod json;
mod node_owned;
mod node_view;
pub mod vec_u8;

pub use crate::iterator::YamlIterator;
pub use crate::json::convert_yaml_to_json;
pub use crate::node_owned::NodeOwned;
pub use crate::node_view::NodeView;
pub use crate::vec_u8::VecU8;
use ordered_float::OrderedFloat;
use saphyr::LoadableYamlNode;
pub use saphyr::YamlOwned;
use std::panic::{self, AssertUnwindSafe};

pub fn load(input: &str) -> Result<NodeOwned, VecU8> {
    // Defense-in-depth at the C++ FFI boundary: callers validate UTF-8 up front
    // without relying on unwinding, and `catch_unwind` converts any unexpected
    // bug in the third-party `saphyr` parser into an `Err` instead of aborting
    // across `extern "C"`.
    match panic::catch_unwind(AssertUnwindSafe(|| YamlOwned::load_from_str(input))) {
        Ok(Ok(docs)) => {
            if docs.is_empty() {
                Err(VecU8::from("Input is empty"))
            } else {
                Ok(NodeOwned::new(YamlOwned::Sequence(docs)))
            }
        }
        Ok(Err(err)) => Err(VecU8::from(err.to_string())),
        Err(_) => Err(VecU8::from("Internal panic while parsing YAML")),
    }
}

pub fn yaml_owned_from_i64(val: i64) -> YamlOwned {
    YamlOwned::Value(saphyr::ScalarOwned::Integer(val))
}

pub fn yaml_owned_from_f64(val: f64) -> YamlOwned {
    YamlOwned::Value(saphyr::ScalarOwned::FloatingPoint(OrderedFloat(val)))
}

pub fn yaml_owned_from_bool(val: bool) -> YamlOwned {
    YamlOwned::Value(saphyr::ScalarOwned::Boolean(val))
}

pub fn yaml_owned_from_str(val: &str) -> YamlOwned {
    YamlOwned::Value(saphyr::ScalarOwned::String(val.to_string()))
}

/// Constructs a `YamlOwned` string scalar from arbitrary bytes, replacing any
/// invalid UTF-8 sequences with `U+FFFD`.
pub fn yaml_owned_from_bytes(val: &[u8]) -> YamlOwned {
    YamlOwned::Value(saphyr::ScalarOwned::String(String::from_utf8_lossy(val).into_owned()))
}

pub fn dump(node: NodeView) -> Result<VecU8, VecU8> {
    // Defense-in-depth at the C++ FFI boundary: convert any unexpected bug in
    // the third-party `saphyr` emitter into an `Err` instead of unwinding
    // across `extern "C"`.
    match panic::catch_unwind(AssertUnwindSafe(|| {
        let mut out = String::new();
        let mut emitter = saphyr::YamlEmitter::new(&mut out);
        let node_inner = node.inner().unwrap_or(&YamlOwned::BadValue);
        let yaml = saphyr::Yaml::from(node_inner);
        match emitter.dump(&yaml) {
            Ok(_) => Ok(VecU8::from(out)),
            Err(e) => Err(VecU8::from(e.to_string())),
        }
    })) {
        Ok(result) => result,
        Err(_) => Err(VecU8::from("Internal panic while dumping YAML")),
    }
}
