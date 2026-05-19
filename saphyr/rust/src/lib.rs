#[macro_use]
mod macros;
mod crubit_util;
mod iterator;
mod node_owned;
mod node_view;
pub use crate::iterator::YamlIterator;
pub use crate::node_owned::{NodeOwned, ResultNodeOwned};
pub use crate::node_view::NodeView;
use ordered_float::OrderedFloat;
use saphyr::LoadableYamlNode;
pub use saphyr::YamlOwned;

pub fn load(input: &str) -> ResultNodeOwned {
    match YamlOwned::load_from_str(input) {
        Ok(docs) => {
            if docs.is_empty() {
                ResultNodeOwned::from_err("Input is empty".to_string().into())
            } else {
                ResultNodeOwned::from_ok(NodeOwned::new(YamlOwned::Sequence(docs)))
            }
        }
        Err(err) => ResultNodeOwned::from_err(err.to_string().into()),
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

make_result_type!(cc_std::std::string, ResultString);

pub fn dump(node: NodeView) -> ResultString {
    let mut out = String::new();
    let mut emitter = saphyr::YamlEmitter::new(&mut out);
    let node_inner = node.inner().unwrap_or(&YamlOwned::BadValue);
    let yaml = saphyr::Yaml::from(node_inner);
    match emitter.dump(&yaml) {
        Ok(_) => ResultString::from_ok(cc_std::std::string::from(out)),
        Err(e) => ResultString::from_err(e.to_string().into()),
    }
}
