use crate::node_view::NodeView;
use saphyr::YamlOwned;

#[derive(Debug)]
pub struct NodeOwned {
    yaml: YamlOwned,
}

impl Default for NodeOwned {
    fn default() -> Self {
        NodeOwned { yaml: YamlOwned::BadValue }
    }
}

impl NodeOwned {
    pub fn new(yaml: YamlOwned) -> Self {
        Self { yaml }
    }

    pub fn inner(&self) -> Option<&YamlOwned> {
        match &self.yaml {
            YamlOwned::BadValue => None,
            _ => Some(&self.yaml),
        }
    }

    yaml_node_common_impl!(NodeOwned);

    pub fn as_view(&self) -> NodeView {
        NodeView::new(&self.yaml)
    }

    pub fn get_at_index<'a>(&'a self, index: usize) -> Option<NodeView<'a>> {
        if let Some(YamlOwned::Sequence(seq)) = self.inner() {
            seq.get(index).map(NodeView::new)
        } else {
            None
        }
    }

    pub fn set_at_index(&mut self, index: usize, value: YamlOwned) {
        if let YamlOwned::Sequence(seq) = &mut self.yaml {
            if index < seq.len() {
                seq[index] = value;
            } else if index == seq.len() {
                seq.push(value);
            }
        }
    }

    pub fn get_at_key<'a>(&'a self, key: &str) -> Option<NodeView<'a>> {
        if let Some(YamlOwned::Mapping(map)) = &self.inner() {
            let key_node = YamlOwned::Value(saphyr::ScalarOwned::String(key.into()));
            map.get(&key_node).map(NodeView::new)
        } else {
            None
        }
    }

    pub fn set_at_key(&mut self, key: &str, value: YamlOwned) {
        if let YamlOwned::Mapping(map) = &mut self.yaml {
            let key_node = YamlOwned::Value(saphyr::ScalarOwned::String(key.into()));
            map.insert(key_node, value);
        }
    }

    pub fn get_key_at_index<'a>(&'a self, index: usize) -> Option<NodeView<'a>> {
        if let Some(YamlOwned::Mapping(map)) = self.inner() {
            map.iter().nth(index).map(|(k, _)| NodeView::new(k))
        } else {
            None
        }
    }

    pub fn get_value_at_index<'a>(&'a self, index: usize) -> Option<NodeView<'a>> {
        if let Some(YamlOwned::Mapping(map)) = self.inner() {
            map.iter().nth(index).map(|(_, v)| NodeView::new(v))
        } else {
            None
        }
    }
}
