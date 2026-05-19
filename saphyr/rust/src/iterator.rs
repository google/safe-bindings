use crate::node_view::NodeView;

// Crubit cannot expose trait objects (`Box<dyn Trait>`) directly to C++
// (go/crubit/overview/limits#abstraction-and-interfaces). We work around this
// by hiding the trait object inside an opaque struct (`YamlIterator`).

/// An `Iterator` extension that adds `clone_box()`, allowing type-erased
/// iterators (`Box<dyn CloneableYamlIterator>`) to be cloned.
pub trait CloneableYamlIterator<'a>: Iterator<Item = (NodeView<'a>, Option<NodeView<'a>>)> {
    fn clone_box(&self) -> Box<dyn CloneableYamlIterator<'a> + 'a>;
}

impl<'a, T> CloneableYamlIterator<'a> for T
where
    T: Iterator<Item = (NodeView<'a>, Option<NodeView<'a>>)> + Clone + 'a,
{
    fn clone_box(&self) -> Box<dyn CloneableYamlIterator<'a> + 'a> {
        Box::new(self.clone())
    }
}

/// An opaque wrapper around `CloneableYamlIterator` exposed to C++ via Crubit.
#[derive(Default)]
pub struct YamlIterator<'a> {
    iter: Option<Box<dyn CloneableYamlIterator<'a> + 'a>>,
}

impl<'a> Clone for YamlIterator<'a> {
    fn clone(&self) -> Self {
        Self { iter: self.iter.as_ref().map(|iter| iter.clone_box()) }
    }
}

impl<'a> YamlIterator<'a> {
    pub fn new(iter: Option<Box<dyn CloneableYamlIterator<'a> + 'a>>) -> Self {
        Self { iter }
    }

    /// Advances the iterator to the next key-value pair and populates the given references.
    /// Returns true if an element was successfully yielded, false otherwise.
    pub fn next(&mut self, key: &mut NodeView<'a>, value: &mut Option<NodeView<'a>>) -> bool {
        if let Some(ref mut iter) = self.iter
            && let Some((k, v)) = iter.next()
        {
            *key = k;
            *value = v;
            return true;
        }
        false
    }
}
