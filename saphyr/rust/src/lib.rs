#[macro_use]
mod macros;
mod iterator;
mod node_owned;
mod node_view;
pub mod vec_u8;

pub use crate::iterator::YamlIterator;
pub use crate::node_owned::NodeOwned;
pub use crate::node_view::NodeView;
pub use crate::vec_u8::VecU8;
use ordered_float::OrderedFloat;
use saphyr::LoadableYamlNode;
pub use saphyr::YamlOwned;

pub fn load(input: &str) -> Result<NodeOwned, VecU8> {
    match YamlOwned::load_from_str(input) {
        Ok(docs) => {
            if docs.is_empty() {
                Err(VecU8::from("Input is empty"))
            } else {
                Ok(NodeOwned::new(YamlOwned::Sequence(docs)))
            }
        }
        Err(err) => Err(VecU8::from(err.to_string())),
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

pub fn dump(node: NodeView) -> Result<VecU8, VecU8> {
    let mut out = String::new();
    let mut emitter = saphyr::YamlEmitter::new(&mut out);
    let node_inner = node.inner().unwrap_or(&YamlOwned::BadValue);
    let yaml = saphyr::Yaml::from(node_inner);
    match emitter.dump(&yaml) {
        Ok(_) => Ok(VecU8::from(out)),
        Err(e) => Err(VecU8::from(e.to_string())),
    }
}
