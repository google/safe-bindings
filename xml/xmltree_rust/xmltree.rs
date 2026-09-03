//! This crate provides C++ bindings for the xmltree crate using an Arena-based document.

use std::sync::atomic::{AtomicU64, Ordering};
use xml::namespace::Namespace;
use xml::writer::{EmitterConfig, XmlEvent};
use xmltree::{Element, XMLNode};

/// The doc_id value that will be assigned to the next XmlDocument instance.
///
/// This counter is atomic to ensure that each instance of XmlDocument, even if they are created
/// concurrently from different threads, will get a different ID.
static NEXT_DOC_ID: AtomicU64 = AtomicU64::new(1);

/// Lightweight handle representing a node inside an `XmlDocument`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct NodeId {
    /// Index in the document's node storage.
    pub index: u32,
    /// The generation of the slot occupied by this node when it was created. This is used in
    /// subsequent accesses to the node to verify that the slot has not been freed and reused by
    /// another node (-> higher generation counter). Starts at 1; a value of 0 indicates an invalid
    /// handle (e.g. from `Default::default()`).
    pub generation: u32,
    /// A 64-bit id for the document, to catch logic errors when mixing nodes across different
    /// `XmlDocument` instances.
    pub doc_id: u64,
}

/// A struct representing an XML attribute, including its key (local name), optional prefix,
/// namespace URI, and value.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Attribute {
    pub key: String,
    pub prefix: Option<String>,
    pub namespace_uri: Option<String>,
    pub value: String,
}

/// Types of XML nodes.
///
/// This enum is intended for C++ compatibility (returned by `node_type`).
/// In pure Rust, matching on the variants of `NodeData` is preferred.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeType {
    #[default]
    Element,
    Comment,
    CData,
    Text,
    ProcessingInstruction,
}

/// Default maximum allowed nesting depth of XML elements, matching `libxml`'s default limit.
pub const DEFAULT_MAX_DEPTH: usize = 256;

/// Simplified error struct for XML parsing operations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParseError {
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

/// Error struct for XML DOM tree manipulation operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeError {
    // The NodeId belongs to a different document.
    DocumentMismatch(NodeId, u64),
    // The given NodeId doesn't exist in the document.
    NodeNotFound(NodeId),
    // The data requested is not present in the node (e.g. calling `node_children` on a `text`
    // node).
    InvalidNodeType,
    // The requested operation would break a tree invariant.
    InvalidOperation(&'static str),
}

impl std::fmt::Display for TreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TreeError::DocumentMismatch(_, _) => {
                write!(f, "Nodes belong to different documents")
            }
            TreeError::NodeNotFound(_) => {
                write!(f, "Node not found in document")
            }
            TreeError::InvalidNodeType => {
                write!(f, "Invalid node type")
            }
            TreeError::InvalidOperation(msg) => {
                write!(f, "{}", msg)
            }
        }
    }
}

impl std::error::Error for TreeError {}

/// Kinds of tree errors.
///
/// This enum is intended for C++ compatibility (returned by the `kind()` method). In pure Rust,
/// matching on the variants of `TreeError` is preferred.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeErrorKind {
    DocumentMismatch,
    NodeNotFound,
    InvalidNodeType,
    InvalidOperation,
}

impl TreeError {
    /// Returns the kind of tree error.
    pub fn kind(&self) -> TreeErrorKind {
        match self {
            TreeError::DocumentMismatch(_, _) => TreeErrorKind::DocumentMismatch,
            TreeError::NodeNotFound(_) => TreeErrorKind::NodeNotFound,
            TreeError::InvalidNodeType => TreeErrorKind::InvalidNodeType,
            TreeError::InvalidOperation(_) => TreeErrorKind::InvalidOperation,
        }
    }
}

/// Data specific to each node type.
#[derive(Debug, Clone)]
enum NodeData {
    Element {
        name: String,
        prefix: Option<String>,
        namespace: Option<String>,
        namespaces: Option<Namespace>,
        attributes: Vec<Attribute>,
        children: Vec<NodeId>,
    },
    Comment(String),
    CData(String),
    Text(String),
    ProcessingInstruction {
        name: String,
        data: Option<String>,
    },
}

/// A node in the XML document.
#[derive(Debug, Clone)]
struct Node {
    /// Common for all nodes.
    parent: Option<NodeId>,
    /// Further data depending on the node type.
    data: NodeData,
}

/// A slot that can hold a document Node.
#[derive(Debug, Clone)]
enum Slot<T> {
    /// An Occupied slot holds node data. The generation counter is used to detect NodeIds that
    /// refer to deleted nodes. Starts at 1; a value of 0 indicates an invalid handle (e.g., from
    /// `Default::default()`).
    Occupied { data: T, generation: u32 },
    /// Free slots are organized in a linked list of indices.
    Free { next_free: Option<u32>, generation: u32 },
    /// A slot gets disabled when it reaches the maximum representable generation count. This
    /// prevents old invalid NodeIds from accidentally becoming valid after generation wraparound.
    Disabled,
}

/// Represents an XML document tree. `XmlDocument` owns all the nodes in the tree, and it can also
/// own nodes in a detached state (typically temporarily, during tree edit operations).
///
/// Note that `XmlDocument` assumes the document has a single root node. The underlying `xmltree`
/// has an `Element::parse_all` function that returns any other top-level nodes that might be
/// present in a document (like comments or processing instructions), but it doesn't have a similar
/// `write_all` function so for now we limit ourselves to storing data that we can write back.
///
/// This class is not thread safe. Creating `XmlDocument` objects concurrently from different
/// threads is supported, but a single `XmlDocument` instance can't be used simultaneously from
/// multiple threads.
#[derive(Debug)]
pub struct XmlDocument {
    /// Document id, to check that NodeIds are always used with their document.
    doc_id: u64,
    /// Storage of slots for document nodes.
    slots: Vec<Slot<Node>>,
    /// Head of the free-list for slot reuse.
    free_head: Option<u32>,
    /// Root element of the document. A value of `None` means the root element has been detached
    /// and no other element has been designated as the root yet.
    root: Option<NodeId>,
}

impl Default for XmlDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlDocument {
    /// Creates a new empty `XmlDocument`.
    pub fn new() -> Self {
        Self {
            doc_id: NEXT_DOC_ID.fetch_add(1, Ordering::Relaxed),
            slots: Vec::new(),
            free_head: None,
            root: None,
        }
    }

    /// Returns the root `NodeId` of the document, if present.
    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    /// Sets the root node. The new root must be a detached `Element`. The old root (if any) will
    /// become detached.
    pub fn set_root(&mut self, new_root_id: NodeId) -> Result<NodeId, TreeError> {
        let new_root = self.find_node(new_root_id)?;
        if !matches!(new_root.data, NodeData::Element { .. }) {
            return Err(TreeError::InvalidNodeType);
        }
        if new_root.parent.is_some() {
            return Err(TreeError::InvalidOperation("set_root argument must be a detached node."));
        }
        self.root = Some(new_root_id);
        Ok(new_root_id)
    }

    /// Finds a node by its NodeId and returns a reference to it. Returns an error if the node
    /// doesn't exist in the document.
    fn find_node(&self, id: NodeId) -> Result<&Node, TreeError> {
        if id.doc_id != self.doc_id {
            log::error!(
                "Mismatched document ID: node ID has doc_id {}, but this document has doc_id {}",
                id.doc_id,
                self.doc_id
            );
            return Err(TreeError::DocumentMismatch(id, self.doc_id));
        }
        match self.slots.get(id.index as usize) {
            Some(Slot::Occupied { data, generation }) if *generation == id.generation => Ok(data),
            _ => Err(TreeError::NodeNotFound(id)),
        }
    }

    /// Finds a node by its NodeId and returns a mutable reference to it. Returns an error if the
    /// node doesn't exist in the document.
    fn find_node_mut(&mut self, id: NodeId) -> Result<&mut Node, TreeError> {
        if id.doc_id != self.doc_id {
            log::error!(
                "Mismatched document ID: node ID has doc_id {}, but this document has doc_id {}",
                id.doc_id,
                self.doc_id
            );
            return Err(TreeError::DocumentMismatch(id, self.doc_id));
        }
        match self.slots.get_mut(id.index as usize) {
            Some(Slot::Occupied { data, generation }) if *generation == id.generation => Ok(data),
            _ => Err(TreeError::NodeNotFound(id)),
        }
    }

    /// Allocates a node and returns its NodeId.
    fn alloc_node(&mut self, node: Node) -> NodeId {
        if let Some(index) = self.free_head {
            let Slot::Free { next_free, generation } = self.slots[index as usize] else {
                unreachable!("Free list pointed to non-free slot");
            };
            self.free_head = next_free;
            self.slots[index as usize] = Slot::Occupied { data: node, generation };
            NodeId { doc_id: self.doc_id, index, generation }
        } else {
            let index = self.slots.len() as u32;
            let generation = 1;
            self.slots.push(Slot::Occupied { data: node, generation });
            NodeId { doc_id: self.doc_id, index, generation }
        }
    }

    /// Returns the `NodeType` of the specified node, or Err if the node doesn't exist in the
    /// document.
    pub fn node_type(&self, id: NodeId) -> Result<NodeType, TreeError> {
        let node = self.find_node(id)?;
        Ok(match &node.data {
            NodeData::Element { .. } => NodeType::Element,
            NodeData::Comment(_) => NodeType::Comment,
            NodeData::CData(_) => NodeType::CData,
            NodeData::Text(_) => NodeType::Text,
            NodeData::ProcessingInstruction { .. } => NodeType::ProcessingInstruction,
        })
    }

    /// Returns the tag name if the node is an Element (or target name for ProcessingInstruction),
    /// or Err if invalid or not an Element/ProcessingInstruction.
    pub fn node_name(&self, id: NodeId) -> Result<&str, TreeError> {
        let node = self.find_node(id)?;
        match &node.data {
            NodeData::Element { name, .. } | NodeData::ProcessingInstruction { name, .. } => {
                Ok(name.as_str())
            }
            _ => Err(TreeError::InvalidNodeType),
        }
    }

    /// Returns the namespace prefix if the node exists and is an Element, or Err if invalid.
    pub fn node_prefix(&self, id: NodeId) -> Result<Option<&str>, TreeError> {
        let node = self.find_node(id)?;
        match &node.data {
            NodeData::Element { prefix, .. } => Ok(prefix.as_deref()),
            _ => Err(TreeError::InvalidNodeType),
        }
    }

    /// Returns the namespace URI if the node exists and is an Element, or Err if invalid.
    pub fn node_namespace(&self, id: NodeId) -> Result<Option<&str>, TreeError> {
        let node = self.find_node(id)?;
        match &node.data {
            NodeData::Element { namespace, .. } => Ok(namespace.as_deref()),
            _ => Err(TreeError::InvalidNodeType),
        }
    }

    /// Returns the attributes of the element if the node exists and is an Element, or Err if
    /// invalid.
    pub fn node_attributes(&self, id: NodeId) -> Result<&[Attribute], TreeError> {
        let node = self.find_node(id)?;
        match &node.data {
            NodeData::Element { attributes, .. } => Ok(attributes.as_slice()),
            _ => Err(TreeError::InvalidNodeType),
        }
    }

    /// Returns the value of the specified attribute matching solely by local name (ignoring
    /// prefix and namespace), or a TreeError if not found. If multiple attributes share the same
    /// local name under different namespaces, returns the first match.
    pub fn get_attribute(&self, id: NodeId, name: &str) -> Result<&str, TreeError> {
        self.find_attribute_by_name(id, name)?
            .ok_or(TreeError::InvalidOperation("Attribute not found"))
    }

    /// Finds the value of the specified attribute matching solely by local name (ignoring prefix
    /// and namespace), returning None if not found. If multiple attributes share the same local
    /// name under different namespaces, returns the first match.
    pub fn find_attribute_by_name(
        &self,
        id: NodeId,
        name: &str,
    ) -> Result<Option<&str>, TreeError> {
        let node = self.find_node(id)?;
        match &node.data {
            NodeData::Element { attributes, .. } => {
                Ok(attributes.iter().find(|a| a.key == name).map(|a| a.value.as_str()))
            }
            _ => Err(TreeError::InvalidNodeType),
        }
    }

    /// Finds the value of the specified attribute by name and namespace URI, returning None if not
    /// found. `None` matches attributes with no namespace. Passing an empty string is invalid and
    /// returns an error; use `None` to match attributes with no namespace.
    pub fn find_attribute_by_name_and_namespace(
        &self,
        id: NodeId,
        name: &str,
        namespace_uri: Option<&str>,
    ) -> Result<Option<&str>, TreeError> {
        if namespace_uri == Some("") {
            return Err(TreeError::InvalidOperation("Attribute namespace URI cannot be empty"));
        }
        let node = self.find_node(id)?;
        match &node.data {
            NodeData::Element { attributes, .. } => Ok(attributes
                .iter()
                .find(|a| a.key == name && a.namespace_uri.as_deref() == namespace_uri)
                .map(|a| a.value.as_str())),
            _ => Err(TreeError::InvalidNodeType),
        }
    }

    /// Returns the child `NodeId`s if the node exists and is an Element, or Err otherwise.
    pub fn node_children(&self, id: NodeId) -> Result<&[NodeId], TreeError> {
        let node = self.find_node(id)?;
        match &node.data {
            NodeData::Element { children, .. } => Ok(children.as_slice()),
            _ => Err(TreeError::InvalidNodeType),
        }
    }

    /// Returns the parent `NodeId` of the node (None for root), or Err if invalid.
    pub fn node_parent(&self, id: NodeId) -> Result<Option<NodeId>, TreeError> {
        let node = self.find_node(id)?;
        Ok(node.parent)
    }

    /// Returns the text content of a text/comment/cdata/pi node, or concatenated text of all
    /// descendant text/cdata nodes for Element nodes. For ProcessingInstruction nodes, returns only
    /// the data portion (excluding the target name). Returns `Err` if invalid.
    pub fn node_text(&self, id: NodeId) -> Result<String, TreeError> {
        let node = self.find_node(id)?;
        match &node.data {
            NodeData::Text(s) | NodeData::Comment(s) | NodeData::CData(s) => Ok(s.clone()),
            NodeData::ProcessingInstruction { data, .. } => Ok(data.clone().unwrap_or_default()),
            NodeData::Element { .. } => {
                let mut text = String::new();
                self.gather_element_text(node, &mut text)?;
                Ok(text)
            }
        }
    }

    fn gather_element_text(&self, node: &Node, out: &mut String) -> Result<(), TreeError> {
        if let NodeData::Element { children, .. } = &node.data {
            for child_id in children {
                let child_node = self.find_node(*child_id)?;
                match &child_node.data {
                    NodeData::Text(s) | NodeData::CData(s) => out.push_str(s),
                    NodeData::Element { .. } => self.gather_element_text(child_node, out)?,
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Finds the first child of a node (given by `id`) that has the specified `name`.
    ///
    /// Returns:
    /// - Ok(Some(child_id)) if a child was found
    /// - Ok(None) if the node doesn't have such a child
    /// - Err if the node doesn't exist or is not an element.
    ///
    /// The result node can be an Element or a ProcessingInstruction, that is, nodes with names.
    pub fn find_child_by_name(&self, id: NodeId, name: &str) -> Result<Option<NodeId>, TreeError> {
        let node = self.find_node(id)?;
        let NodeData::Element { children, .. } = &node.data else {
            return Err(TreeError::InvalidNodeType);
        };
        for child_id in children {
            let child_node =
                self.find_node(*child_id).expect("Internal tree corruption: child node not found");
            let child_name = match &child_node.data {
                NodeData::Element { name, .. } | NodeData::ProcessingInstruction { name, .. } => {
                    name
                }
                _ => continue, // skip unnamed nodes
            };
            // NOTE(b/510287837): Add support for prefixes and namespaces.
            if child_name == name {
                return Ok(Some(*child_id));
            }
        }
        Ok(None)
    }

    /// Checks if adding `child_id` as a child of `parent_id` would create a cycle.
    fn check_cycle(&self, parent_id: NodeId, child_id: NodeId) -> Result<(), TreeError> {
        let mut curr = Some(parent_id);
        while let Some(curr_id) = curr {
            if curr_id == child_id {
                return Err(TreeError::InvalidOperation(
                    "Cannot add child: operation would create a cycle",
                ));
            }
            curr = self.find_node(curr_id).ok().and_then(|n| n.parent);
        }
        Ok(())
    }

    /// Adds an existing detached node as a child of `parent_id`.
    ///
    /// The following requirements are enforced:
    /// - The new parent must be an `Element` (it can't be, for example, a comment).
    /// - `child_id` must belong to the same document as `parent_id`.
    /// - `child_id` must be a detached node. That is, `child_id` can't already have a parent, and
    ///   it can't be the document root.
    /// - `child_id` must not be an ancestor of `parent_id` (that is, the insertion must not
    ///   introduce a cycle).
    ///
    /// Returns `Ok(child_id)` on success, or `Err(TreeError)` describing the failure if any
    /// requirement is violated.
    pub fn add_child(&mut self, parent_id: NodeId, child_id: NodeId) -> Result<NodeId, TreeError> {
        // find_node will check that both ids belong to the current document.
        let parent = self.find_node(parent_id)?;
        let child = self.find_node(child_id)?;

        // Child must be detached.
        if child.parent.is_some() {
            return Err(TreeError::InvalidOperation("Child node already has a parent"));
        }

        if Some(child_id) == self.root {
            return Err(TreeError::InvalidOperation("Root node can't be a child"));
        }

        // Parent must be an Element
        if !matches!(parent.data, NodeData::Element { .. }) {
            return Err(TreeError::InvalidNodeType);
        }

        self.check_cycle(parent_id, child_id)?;

        // Apply mutation
        self.find_node_mut(child_id)?.parent = Some(parent_id);
        if let NodeData::Element { children, .. } = &mut self.find_node_mut(parent_id)?.data {
            children.push(child_id);
        }

        Ok(child_id)
    }

    /// Inserts a detached node as a child of `parent_id`, immediately after `ref_child_id`.
    ///
    /// The following requirements are enforced:
    /// - The new parent must be an `Element`.
    /// - `child_id` must belong to the same document as `parent_id`.
    /// - `child_id` must be a detached node. That is, `child_id` can't already have a parent, and
    ///   it can't be the document root.
    /// - `ref_child_id` must be a child of `parent_id`.
    /// - `child_id` must not be an ancestor of `parent_id` (that is, the insertion must not
    ///   introduce a cycle).
    ///
    /// Returns `Ok(child_id)` on success, or `Err(TreeError)` describing the failure if any
    /// requirement is violated.
    pub fn insert_child_after(
        &mut self,
        parent_id: NodeId,
        child_id: NodeId,
        ref_child_id: NodeId,
    ) -> Result<NodeId, TreeError> {
        let parent = self.find_node(parent_id)?;
        let child = self.find_node(child_id)?;
        self.find_node(ref_child_id)?; // Ensure ref_child exists

        if child.parent.is_some() {
            return Err(TreeError::InvalidOperation("Child node already has a parent"));
        }

        if Some(child_id) == self.root {
            return Err(TreeError::InvalidOperation("Root node can't be a child"));
        }

        if !matches!(parent.data, NodeData::Element { .. }) {
            return Err(TreeError::InvalidNodeType);
        }

        let is_ref_child = if let NodeData::Element { children, .. } = &parent.data {
            children.contains(&ref_child_id)
        } else {
            false
        };
        if !is_ref_child {
            return Err(TreeError::InvalidOperation("Reference node is not a child of the parent"));
        }

        self.check_cycle(parent_id, child_id)?;

        // Apply mutation
        self.find_node_mut(child_id)?.parent = Some(parent_id);
        if let NodeData::Element { children, .. } = &mut self.find_node_mut(parent_id)?.data {
            let pos = children.iter().position(|&id| id == ref_child_id).unwrap();
            children.insert(pos + 1, child_id);
        }

        Ok(child_id)
    }

    /// Detaches a node from its parent, converting it into a root of a detached tree.
    ///
    /// If the node has a parent, it is removed from the parent's children list and its parent
    /// pointer is cleared (`None`). If the node is the root of the document, `self.root` is
    /// cleared.
    ///
    /// Returns the id of the detached node.
    pub fn detach_node(&mut self, id: NodeId) -> Result<NodeId, TreeError> {
        let node = self.find_node(id)?;

        // Unlink from parent if present
        if let Some(parent_id) = node.parent {
            let parent_node = self
                .find_node_mut(parent_id)
                .expect("Internal tree corruption: parent node must exist");
            let NodeData::Element { children, .. } = &mut parent_node.data else {
                panic!("Internal tree corruption: parent node must be an element");
            };
            children.retain(|&child| child != id);

            let target_node = self.find_node_mut(id)?;
            target_node.parent = None;
        }

        // Clear root if detaching the root node
        if let Some(root) = self.root
            && root == id
        {
            self.root = None;
        }

        Ok(id)
    }

    /// Deletes a node and all its descendants from the document.
    ///
    /// If the node has a parent, it is removed from the parent's children list.
    /// If the node is the root, the document's root is cleared.
    /// Free slots are placed on the free list for reuse, unless their generation counter
    /// reaches `u32::MAX`, in which case they are permanently disabled.
    ///
    /// Returns the (now invalid) NodeId of the deleted node.
    pub fn delete_node(&mut self, id: NodeId) -> Result<NodeId, TreeError> {
        self.detach_node(id)?;
        self.delete_detached_node(id);
        Ok(id)
    }

    /// Recursively frees a node and all its descendants without updating any parent pointers.
    ///
    /// Useful for internal recursive deletion of subtrees where the parent is already being deleted
    /// (avoiding redundant parent unlinking lookups), or for deleting detached nodes that have no
    /// parent.
    fn delete_detached_node(&mut self, id: NodeId) {
        let node = self.take_slot(id).expect("Internal tree corruption: node to delete must exist");
        let element_children = match node.data {
            NodeData::Element { children, .. } => Some(children),
            _ => None,
        };

        if let Some(children) = element_children {
            for child_id in children {
                self.delete_detached_node(child_id);
            }
        }
    }

    /// Extracts a node from a slot and marks the slot as free (or disabled).
    fn take_slot(&mut self, id: NodeId) -> Result<Node, TreeError> {
        if id.doc_id != self.doc_id {
            return Err(TreeError::DocumentMismatch(id, self.doc_id));
        }
        let index = id.index as usize;
        let current_gen = match self.slots.get(index) {
            Some(Slot::Occupied { generation, .. }) => *generation,
            _ => return Err(TreeError::NodeNotFound(id)),
        };

        if current_gen != id.generation {
            return Err(TreeError::NodeNotFound(id));
        }

        let next_gen = current_gen.saturating_add(1);
        let new_slot = if next_gen == u32::MAX {
            Slot::Disabled
        } else {
            let slot = Slot::Free { next_free: self.free_head, generation: next_gen };
            self.free_head = Some(id.index);
            slot
        };

        let old_slot = std::mem::replace(&mut self.slots[index], new_slot);
        if let Slot::Occupied { data, .. } = old_slot {
            Ok(data)
        } else {
            unreachable!("Verified above that it was Occupied");
        }
    }

    /// Recursively inserts a parsed `xmltree::XMLNode` and its children into the current document.
    fn insert_parsed_node(&mut self, parsed_node: XMLNode, parent_id: Option<NodeId>) -> NodeId {
        // If `parsed_node` is an Element, we'll save its children here until we insert the node
        // and have a parent_id for the children.
        let mut elem_children: Option<Vec<XMLNode>> = None;

        let data = match parsed_node {
            XMLNode::Comment(s) => NodeData::Comment(s),
            XMLNode::CData(s) => NodeData::CData(s),
            XMLNode::Text(s) => NodeData::Text(s),
            XMLNode::ProcessingInstruction(name, pi_data) => {
                NodeData::ProcessingInstruction { name, data: pi_data }
            }
            XMLNode::Element(elem) => {
                elem_children = Some(elem.children);

                let attributes = elem
                    .attributes
                    .into_iter()
                    .map(|(k, v)| Attribute {
                        key: k.local_name,
                        prefix: k.prefix,
                        namespace_uri: k.namespace,
                        value: v,
                    })
                    .collect();
                NodeData::Element {
                    name: elem.name,
                    prefix: elem.prefix,
                    namespace: elem.namespace,
                    namespaces: elem.namespaces.map(|ns| Namespace(ns.0)),
                    attributes,
                    children: Vec::new(),
                }
            }
        };

        let node = Node { parent: parent_id, data };
        let id = self.alloc_node(node);

        // If the parsed node was an element, traverse its children and add them recursively.
        if let Some(parsed_children) = elem_children {
            for child in parsed_children {
                let child_id = self.insert_parsed_node(child, Some(id));
                let NodeData::Element { children, .. } = &mut self
                    .find_node_mut(id)
                    .expect("Node that was just added must exist, tree is corrupted.")
                    .data
                else {
                    unreachable!();
                };
                children.push(child_id);
            }
        }

        id
    }

    /// Creates a new, detached Element node in the document.
    pub fn create_element(&mut self, name: &str) -> NodeId {
        self.alloc_node(Node {
            parent: None,
            data: NodeData::Element {
                name: name.to_string(),
                prefix: None,
                namespace: None,
                namespaces: None,
                attributes: Vec::new(),
                children: Vec::new(),
            },
        })
    }

    /// Creates a new, detached Text node in the document.
    pub fn create_text_node(&mut self, content: &str) -> NodeId {
        self.alloc_node(Node { parent: None, data: NodeData::Text(content.to_string()) })
    }

    /// Sets or updates an attribute without prefix/namespace on an Element node.
    pub fn set_attribute(
        &mut self,
        id: NodeId,
        key: &str,
        value: &str,
    ) -> Result<NodeId, TreeError> {
        self.set_attribute_with_ns(id, key, None, None, value)
    }

    /// Sets or updates an attribute with an optional prefix and namespace URI on an Element node.
    /// `prefix` and `namespace_uri` must either both be `Some` (with non-empty strings) or both
    /// be `None`. Passing an empty string for either parameter is invalid and returns an error.
    pub fn set_attribute_with_ns(
        &mut self,
        id: NodeId,
        key: &str,
        prefix: Option<&str>,
        namespace_uri: Option<&str>,
        value: &str,
    ) -> Result<NodeId, TreeError> {
        if prefix.is_some() != namespace_uri.is_some() {
            return Err(TreeError::InvalidOperation(
                "Attribute prefix and namespace_uri must both be set or both be unset",
            ));
        }
        if prefix == Some("") {
            return Err(TreeError::InvalidOperation("Attribute prefix cannot be empty"));
        }
        if namespace_uri == Some("") {
            return Err(TreeError::InvalidOperation("Attribute namespace URI cannot be empty"));
        }
        let node = self.find_node_mut(id)?;
        if let NodeData::Element { attributes, .. } = &mut node.data {
            for attr in attributes.iter_mut() {
                if attr.key == key && attr.namespace_uri.as_deref() == namespace_uri {
                    attr.prefix = prefix.map(|s| s.to_string());
                    attr.value = value.to_string();
                    return Ok(id);
                }
            }
            attributes.push(Attribute {
                key: key.to_string(),
                prefix: prefix.map(|s| s.to_string()),
                namespace_uri: namespace_uri.map(|s| s.to_string()),
                value: value.to_string(),
            });
            Ok(id)
        } else {
            Err(TreeError::InvalidNodeType)
        }
    }

    /// Deletes all attributes matching solely by local name (ignoring prefix and namespace) from
    /// an Element node.
    pub fn delete_attribute(&mut self, id: NodeId, key: &str) -> Result<NodeId, TreeError> {
        let node = self.find_node_mut(id)?;
        if let NodeData::Element { attributes, .. } = &mut node.data {
            attributes.retain(|attr| attr.key != key);
            Ok(id)
        } else {
            Err(TreeError::InvalidNodeType)
        }
    }

    /// Deletes an attribute from an Element node matching the key and optional namespace URI.
    /// `None` matches attributes with no namespace. Passing an empty string is invalid and returns
    /// an error; use `None` to match attributes with no namespace.
    pub fn delete_attribute_by_name_and_namespace(
        &mut self,
        id: NodeId,
        name: &str,
        namespace_uri: Option<&str>,
    ) -> Result<NodeId, TreeError> {
        if namespace_uri == Some("") {
            return Err(TreeError::InvalidOperation("Attribute namespace URI cannot be empty"));
        }
        let node = self.find_node_mut(id)?;
        if let NodeData::Element { attributes, .. } = &mut node.data {
            attributes.retain(|attr| {
                !(attr.key == name && attr.namespace_uri.as_deref() == namespace_uri)
            });
            Ok(id)
        } else {
            Err(TreeError::InvalidNodeType)
        }
    }

    /// Sets the name of an Element node.
    pub fn set_element_name(&mut self, id: NodeId, name: &str) -> Result<NodeId, TreeError> {
        let node = self.find_node_mut(id)?;
        if let NodeData::Element { name: n, .. } = &mut node.data {
            *n = name.to_string();
            Ok(id)
        } else {
            Err(TreeError::InvalidNodeType)
        }
    }

    /// Sets the text content of a Text, Comment, or CData node.
    pub fn set_text(&mut self, id: NodeId, text: &str) -> Result<NodeId, TreeError> {
        let node = self.find_node_mut(id)?;
        match &mut node.data {
            NodeData::Text(s) | NodeData::Comment(s) | NodeData::CData(s) => {
                *s = text.to_string();
                Ok(id)
            }
            _ => Err(TreeError::InvalidNodeType),
        }
    }

    /// Recursively creates a deep copy of a node and all its descendants.
    /// The cloned subtree is detached.
    pub fn clone_node(&mut self, id: NodeId) -> Result<NodeId, TreeError> {
        let node_data = self.find_node(id)?.data.clone();

        let mut cloned_children = Vec::new();
        if let NodeData::Element { ref children, .. } = node_data {
            for child_id in children {
                cloned_children.push(self.clone_node(*child_id)?);
            }
        }

        let new_data = match node_data {
            NodeData::Element { name, prefix, namespace, namespaces, attributes, .. } => {
                NodeData::Element {
                    name,
                    prefix,
                    namespace,
                    namespaces,
                    attributes,
                    children: Vec::new(),
                }
            }
            other => other,
        };

        let new_id = self.alloc_node(Node { parent: None, data: new_data });

        for &child_id in &cloned_children {
            self.find_node_mut(child_id)?.parent = Some(new_id);
        }

        if let NodeData::Element { children, .. } = &mut self.find_node_mut(new_id)?.data {
            *children = cloned_children;
        }

        Ok(new_id)
    }

    fn to_xmltree_element(&self, id: NodeId) -> Result<Element, TreeError> {
        let node = self.find_node(id)?;
        match &node.data {
            NodeData::Element { name, prefix, namespace, namespaces, attributes, children } => {
                let mut elem = Element::new(name.as_str());
                elem.prefix = prefix.clone();
                elem.namespace = namespace.clone();
                elem.namespaces = namespaces.clone();
                for attr in attributes {
                    let attr_name = xmltree::AttributeName {
                        local_name: attr.key.clone(),
                        namespace: attr.namespace_uri.clone(),
                        prefix: attr.prefix.clone(),
                    };
                    elem.attributes.insert(attr_name, attr.value.clone());
                }
                for child_id in children {
                    elem.children.push(self.to_xmltree_node(*child_id)?);
                }
                Ok(elem)
            }
            _ => Err(TreeError::InvalidNodeType),
        }
    }

    fn to_xmltree_node(&self, id: NodeId) -> Result<XMLNode, TreeError> {
        let node = self.find_node(id)?;
        match &node.data {
            NodeData::Element { .. } => Ok(XMLNode::Element(self.to_xmltree_element(id)?)),
            NodeData::Comment(s) => Ok(XMLNode::Comment(s.clone())),
            NodeData::CData(s) => Ok(XMLNode::CData(s.clone())),
            NodeData::Text(s) => Ok(XMLNode::Text(s.clone())),
            NodeData::ProcessingInstruction { name, data } => {
                Ok(XMLNode::ProcessingInstruction(name.clone(), data.clone()))
            }
        }
    }

    /// Serializes the entire document to a string.
    pub fn serialize_document(&self) -> Result<String, TreeError> {
        let root_id = self.root.ok_or(TreeError::InvalidOperation("Document has no root"))?;
        let root_elem = self.to_xmltree_element(root_id)?;
        let mut buf = Vec::new();
        root_elem.write(&mut buf).map_err(|_| TreeError::InvalidOperation("Write error"))?;
        String::from_utf8(buf).map_err(|_| TreeError::InvalidOperation("Invalid UTF-8"))
    }

    /// Serializes a specific node and its descendants to a string.
    pub fn serialize_node(&self, id: NodeId) -> Result<String, TreeError> {
        let node = self.find_node(id)?;
        let mut buf = Vec::new();
        let config = EmitterConfig::new().write_document_declaration(false).perform_indent(false);

        match &node.data {
            NodeData::Element { .. } => {
                let elem = self.to_xmltree_element(id)?;
                elem.write_with_config(&mut buf, config)
                    .map_err(|_| TreeError::InvalidOperation("Write error"))?;
            }
            NodeData::Text(s) => {
                config
                    .create_writer(&mut buf)
                    .write(XmlEvent::characters(s.as_str()))
                    .map_err(|_| TreeError::InvalidOperation("Write error"))?;
            }
            NodeData::Comment(s) => {
                config
                    .create_writer(&mut buf)
                    .write(XmlEvent::comment(s.as_str()))
                    .map_err(|_| TreeError::InvalidOperation("Write error"))?;
            }
            NodeData::CData(s) => {
                config
                    .create_writer(&mut buf)
                    .write(XmlEvent::cdata(s.as_str()))
                    .map_err(|_| TreeError::InvalidOperation("Write error"))?;
            }
            NodeData::ProcessingInstruction { name, data } => {
                config
                    .create_writer(&mut buf)
                    .write(XmlEvent::processing_instruction(name.as_str(), data.as_deref()))
                    .map_err(|_| TreeError::InvalidOperation("Write error"))?;
            }
        }
        String::from_utf8(buf).map_err(|_| TreeError::InvalidOperation("Invalid UTF-8"))
    }
}

fn parser_config() -> xmltree::ParserConfig {
    xmltree::ParserConfig::new().ignore_comments(false).whitespace_to_characters(true)
}

fn parse_element(
    data: &[u8],
    scanner_max_depth: usize,
    reported_max_depth: usize,
) -> Result<Element, ParseError> {
    let config = parser_config();
    let depth_check = nesting_depth::check_nesting_depth(data, scanner_max_depth, config.clone());
    if depth_check == Err(nesting_depth::NestingDepthError::ExceededMaxDepth) {
        return Err(ParseError {
            message: format!("XML exceeded maximum nesting depth of {}", reported_max_depth),
        });
    }

    let parsed_root = Element::parse_with_config(data, config)
        .map_err(|e| ParseError { message: e.to_string() })?;

    if depth_check.is_err() {
        return Err(ParseError { message: "XML failed to parse".to_string() });
    }

    Ok(parsed_root)
}

/// Parses the given XML byte slice into an `XmlDocument` arena.
pub fn parse(data: &[u8], max_depth: usize) -> Result<XmlDocument, ParseError> {
    let parsed_root = parse_element(data, max_depth, max_depth)?;

    let mut doc = XmlDocument::new();
    let root_id = doc.insert_parsed_node(XMLNode::Element(parsed_root), None);
    doc.root = Some(root_id);

    Ok(doc)
}

/// Parses an XML fragment and inserts the resulting nodes into the document as children
/// of a new, detached dummy element. Returns the NodeId of the dummy element.
///
/// # Why a Dummy Root?
/// Standard XML parsers strictly enforce the rule that an XML document must have exactly
/// one root element. They will throw a fatal error if given a "fragment" (e.g., multiple
/// sibling elements like `<p>one</p><p>two</p>` without an enclosing parent).
///
/// To bypass this and allow callers to instantiate subtrees directly from strings, this
/// function wraps the fragment in a `<__dummy_root__>` element to satisfy the parser.
/// The caller (e.g., C++ over FFI) receives the `NodeId` of this dummy root, and can then
/// extract its children, transplant them into the actual document, and safely delete the
/// dummy root. Returning a single `NodeId` is also safer and simpler across the FFI
/// boundary than returning a dynamic `Vec<NodeId>`.
pub fn parse_fragment(
    doc: &mut XmlDocument,
    data: &[u8],
    max_depth: usize,
) -> Result<NodeId, ParseError> {
    let mut wrapped = b"<__dummy_root__>".to_vec();
    wrapped.extend_from_slice(data);
    wrapped.extend_from_slice(b"</__dummy_root__>");

    // Account for the synthetic `<__dummy_root__>` wrapper element so `max_depth` applies to the
    // fragment's own element nesting depth.
    let parsed_root = parse_element(wrapped.as_slice(), max_depth.saturating_add(1), max_depth)?;

    Ok(doc.insert_parsed_node(XMLNode::Element(parsed_root), None))
}
