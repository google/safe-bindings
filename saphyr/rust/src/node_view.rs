//! A view of a YAML node that does not own its data.
use crate::iterator::{CloneableYamlIterator, YamlIterator};
use crate::node_owned::NodeOwned;
use saphyr::{ScalarOwned, YamlOwned};

#[derive(Clone, Copy, Debug)]
pub struct NodeView<'a> {
    yaml: &'a YamlOwned,
}

impl<'a> Default for NodeView<'a> {
    fn default() -> Self {
        Self::new(&YamlOwned::BadValue)
    }
}

impl<'a> NodeView<'a> {
    pub fn new(yaml: &'a YamlOwned) -> Self {
        Self { yaml }
    }

    pub fn inner(&self) -> Option<&'a YamlOwned> {
        match self.yaml {
            YamlOwned::BadValue => None,
            _ => Some(self.yaml),
        }
    }

    pub fn to_owned(&self) -> NodeOwned {
        NodeOwned::new(self.yaml.clone())
    }

    yaml_node_common_impl!(NodeView<'a>);

    pub fn get_at_index(&self, index: usize) -> Option<NodeView<'a>> {
        if let Some(YamlOwned::Sequence(seq)) = self.inner()
            && let Some(yaml) = seq.get(index)
        {
            Some(Self::new(yaml))
        } else {
            None
        }
    }

    pub fn get_at_key(&self, key: &str) -> Option<NodeView<'a>> {
        if let Some(YamlOwned::Mapping(map)) = self.inner() {
            let key_node = YamlOwned::Value(ScalarOwned::String(key.into()));
            map.get(&key_node).map(Self::new)
        } else {
            None
        }
    }

    pub fn get_key_at_index(&self, index: usize) -> Option<NodeView<'a>> {
        if let Some(YamlOwned::Mapping(map)) = self.inner() {
            map.iter().nth(index).map(|(k, _)| Self::new(k))
        } else {
            None
        }
    }

    pub fn get_value_at_index(&self, index: usize) -> Option<NodeView<'a>> {
        if let Some(YamlOwned::Mapping(map)) = self.inner() {
            map.iter().nth(index).map(|(_, v)| Self::new(v))
        } else {
            None
        }
    }

    /// Yields a new `YamlIterator` which can lazily evaluate map or sequence elements.
    pub fn get_iterator(&self) -> YamlIterator<'a> {
        let yaml = match self.inner() {
            Some(y) => y,
            None => return YamlIterator::default(),
        };

        // The second element of the tuple is `Option<NodeView>` because sequences in YAML do not
        // have keys.
        let iter: Box<dyn CloneableYamlIterator<'a> + 'a> = match yaml {
            YamlOwned::Sequence(seq) => Box::new(seq.iter().map(|v| (NodeView::new(v), None))),
            YamlOwned::Mapping(map) => {
                Box::new(map.iter().map(|(k, v)| (NodeView::new(k), Some(NodeView::new(v)))))
            }
            _ => return YamlIterator::default(),
        };

        YamlIterator::new(Some(iter))
    }
}
