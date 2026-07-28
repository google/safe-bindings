use serde::Serialize;
use serde_json::{Number, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

pub type Status = status_wrapper::StatusWrapper;

#[inline(always)]
fn ok() -> Status {
    status::OkStatus().into()
}

#[inline(always)]
fn internal_error(message: impl Into<String>) -> Status {
    status::internal(message.into()).into()
}

#[inline(always)]
fn invalid_argument_error(message: impl Into<String>) -> Status {
    status::invalid_argument(message.into()).into()
}

#[inline(always)]
fn out_of_range_error(message: impl Into<String>) -> Status {
    status::out_of_range(message.into()).into()
}

#[inline(always)]
fn failed_precondition_error(message: impl Into<String>) -> Status {
    status::failed_precondition(message.into()).into()
}

#[inline(always)]
fn invalid_handle_error(message: impl Into<String>) -> Status {
    status::invalid_argument(message.into()).into()
}

/// An opaque 64-bit integer handle representing a specific node within a JSON document tree.
#[repr(C)]
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeHandle {
    id: u64,
}

impl NodeHandle {
    pub fn root() -> Self {
        Self { id: 0 }
    }
}

/// Represents a single edge/step from a parent JSON node to a child node (object field or array index).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum PathElement {
    Root,
    Field(Box<str>),
    Index(usize),
}

/// A lookup key for child node handles, identifying a child by its parent handle ID and the step taken from parent to child.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PathKey {
    parent_id: NodeHandle,
    element: PathElement,
}

/// Metadata stored for each handle, tracking its parent handle, path element step, and an optional cached pointer into the AST.
#[derive(Clone, Debug)]
struct HandleData {
    parent_id: NodeHandle,
    element: PathElement,
    ptr: Cell<*const Value>,
}

impl PartialEq for HandleData {
    fn eq(&self, other: &Self) -> bool {
        self.parent_id == other.parent_id && self.element == other.element
    }
}

impl Eq for HandleData {}

/// Registry maintaining bi-directional mapping between handles, their parent relationships, and cached AST pointers.
#[derive(Clone, Debug, PartialEq, Eq)]
struct HandleRegistry {
    handle_to_data: RefCell<Vec<HandleData>>,
    key_to_handle: RefCell<HashMap<PathKey, NodeHandle>>,
    owner_addr: Cell<usize>,
}

impl Default for HandleRegistry {
    fn default() -> Self {
        let root_handle = NodeHandle::root();
        let root_data = HandleData {
            parent_id: root_handle,
            element: PathElement::Root,
            ptr: Cell::new(std::ptr::null()),
        };
        let root_key = PathKey { parent_id: root_handle, element: PathElement::Root };
        let mut key_to_handle = HashMap::new();
        key_to_handle.insert(root_key, root_handle);
        Self {
            handle_to_data: RefCell::new(vec![root_data]),
            key_to_handle: RefCell::new(key_to_handle),
            owner_addr: Cell::new(0),
        }
    }
}

impl HandleRegistry {
    pub fn get_or_create_child_handle(
        &self,
        parent_id: NodeHandle,
        element: PathElement,
        ptr: *const Value,
    ) -> Result<NodeHandle, Status> {
        let key = PathKey { parent_id, element };
        let mut key_to_handle = self.key_to_handle.borrow_mut();
        if let Some(&handle) = key_to_handle.get(&key) {
            let handle_to_data = self.handle_to_data.borrow();
            if let Some(data) = handle_to_data.get(handle.id as usize) {
                // Updating `data.ptr` is safe because the handle refers to the same immutable node
                // in `self.value`. The cached pointer may have been invalidated (set to null) by a mutation or clone.
                data.ptr.set(ptr);
            }
            return Ok(handle);
        }
        let mut handle_to_data = self.handle_to_data.borrow_mut();
        let id = handle_to_data.len() as u64;
        if id == u64::MAX {
            return Err(failed_precondition_error(
                "Handle registry overflow: exceeded 2^64 unique handles",
            ));
        }
        let handle = NodeHandle { id };
        handle_to_data.push(HandleData {
            parent_id,
            element: key.element.clone(),
            ptr: Cell::new(ptr),
        });
        key_to_handle.insert(key, handle);
        Ok(handle)
    }

    pub fn invalidate_ptrs(&self) {
        self.owner_addr.set(0);
        let handle_to_data = self.handle_to_data.borrow();
        for data in handle_to_data.iter() {
            data.ptr.set(std::ptr::null());
        }
    }
}

/// A JSON document with an opaque handle-based interface for navigation and modification.
/// Note: `FastSerdeJson` is NOT thread-safe. Its internal `HandleRegistry` uses `RefCell`
/// and `Cell` for caching pointer lookups. Concurrent access from multiple threads will lead
/// to runtime panics or undefined behavior.
pub struct FastSerdeJson {
    value: Value,
    registry: HandleRegistry,
}

impl Clone for FastSerdeJson {
    fn clone(&self) -> Self {
        let cloned = Self { value: self.value.clone(), registry: self.registry.clone() };
        cloned.registry.invalidate_ptrs();
        cloned
    }
}

// Note on CppVec* newtype wrappers around `cc_std::std::vector`:
// While using `cc_std::std::vector<T>` directly across FFI boundaries works cleanly
// for straightforward parameters and direct returns, returning `Result<cc_std::std::vector<T>, Status>`
// currently triggers a Crubit binding generation limitation (b/259749095 — "Generic types are not
// supported yet inside Result"). By defining concrete, non-generic newtype structs and annotating
// them with `#[crubit_annotate::cpp_layout_equivalent]`, we bypass the generic Result error in
// Rust while Crubit seamlessly maps them to standard `std::vector` in C++ without wrapper overhead.

#[crubit_annotate::cpp_layout_equivalent(
    cpp_type = "::std::vector<::std::uint8_t>",
    include_path = "<vector>"
)]
#[derive(Default)]
#[repr(C)]
pub struct CppVecU8(pub cc_std::std::vector<u8>);

impl CppVecU8 {
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<Vec<u8>> for CppVecU8 {
    fn from(v: Vec<u8>) -> Self {
        Self(v.into())
    }
}

impl From<&[u8]> for CppVecU8 {
    fn from(s: &[u8]) -> Self {
        Self(s.into())
    }
}

#[crubit_annotate::cpp_layout_equivalent(
    cpp_type = "::std::vector<::std::vector<::std::uint8_t>>",
    include_path = "<vector>"
)]
#[repr(C)]
pub struct CppVecVecU8(pub cc_std::std::vector<cc_std::std::vector<u8>>);

impl std::iter::FromIterator<CppVecU8> for CppVecVecU8 {
    fn from_iter<I: IntoIterator<Item = CppVecU8>>(iter: I) -> Self {
        Self(iter.into_iter().map(|v| v.0).collect())
    }
}

#[crubit_annotate::cpp_layout_equivalent(
    cpp_type = "::std::vector<::fast_serde_json_rs::NodeHandle>",
    include_path = "<vector>"
)]
#[repr(C)]
pub struct CppVecNodeHandle(pub cc_std::std::vector<NodeHandle>);

impl From<Vec<NodeHandle>> for CppVecNodeHandle {
    fn from(v: Vec<NodeHandle>) -> Self {
        Self(v.into())
    }
}

#[crubit_annotate::cpp_layout_equivalent(
    cpp_type = "::std::vector<::fast_serde_json_rs::FastSerdeJson>",
    include_path = "<vector>"
)]
#[repr(C)]
pub struct CppVecFastSerdeJson(pub cc_std::std::vector<FastSerdeJson>);

impl IntoIterator for CppVecFastSerdeJson {
    type Item = FastSerdeJson;
    type IntoIter = <cc_std::std::vector<FastSerdeJson> as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Default for FastSerdeJson {
    fn default() -> Self {
        Self::new_null()
    }
}

impl From<i64> for FastSerdeJson {
    fn from(v: i64) -> Self {
        Self::from_i64(v)
    }
}

impl From<bool> for FastSerdeJson {
    fn from(v: bool) -> Self {
        Self::from_bool(v)
    }
}

impl TryFrom<f64> for FastSerdeJson {
    type Error = Status;

    fn try_from(v: f64) -> Result<Self, Self::Error> {
        Self::try_from_f64(v)
    }
}

impl TryFrom<&[u8]> for FastSerdeJson {
    type Error = Status;

    fn try_from(v: &[u8]) -> Result<Self, Self::Error> {
        Self::try_from_utf8(v)
    }
}

impl PartialEq for FastSerdeJson {
    fn eq(&self, other: &Self) -> bool {
        self.is_json_equal(other)
    }
}

impl Eq for FastSerdeJson {}

impl std::fmt::Debug for FastSerdeJson {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "FastSerdeJson({:?})", self.value)
    }
}

macro_rules! traverse_path_impl {
    ($root:expr, $path:expr, $as_array:ident, $get_elem:ident, $get_field:ident) => {{
        let mut curr = $root;
        for elem in $path {
            match elem {
                PathElement::Root => {}
                PathElement::Index(idx) => match curr.$as_array() {
                    Some(arr) => {
                        let arr_len = arr.len();
                        match arr.$get_elem(*idx) {
                            Some(val) => curr = val,
                            None => {
                                return Err(invalid_handle_error(format!(
                                    "Index {} out of bounds for array of length {}",
                                    idx, arr_len
                                )));
                            }
                        }
                    }
                    None => {
                        return Err(invalid_handle_error(
                            "Path element is index, but current JSON node is not an array",
                        ));
                    }
                },
                PathElement::Field(name) => match curr.$get_field(name.as_ref()) {
                    Some(val) => curr = val,
                    None => {
                        return Err(invalid_handle_error(format!(
                            "Field '{}' not found in JSON object",
                            name
                        )));
                    }
                },
            }
        }
        Ok(curr)
    }};
}

fn traverse_path<'a>(root: &'a Value, path: &[PathElement]) -> Result<&'a Value, Status> {
    traverse_path_impl!(root, path, as_array, get, get)
}

fn traverse_path_mut<'a>(
    root: &'a mut Value,
    path: &[PathElement],
) -> Result<&'a mut Value, Status> {
    traverse_path_impl!(root, path, as_array_mut, get_mut, get_mut)
}

struct PathFromAncestor {
    ancestor: NodeHandle,
    path: Vec<PathElement>,
}

impl FastSerdeJson {
    fn new(value: Value) -> Self {
        Self { value, registry: HandleRegistry::default() }
    }

    fn get_path_from_ancestor(&self, handle: NodeHandle) -> Result<PathFromAncestor, Status> {
        let handle_to_data = self.registry.handle_to_data.borrow();
        let mut curr_id = handle.id as usize;
        let mut elements = Vec::new();
        while curr_id != 0 {
            let data = handle_to_data.get(curr_id).ok_or_else(|| {
                invalid_handle_error(format!("Invalid NodeHandle ID: {}", curr_id))
            })?;

            // Early-stop if we discover a valid cached ancestor pointer (excluding target handle itself)
            if curr_id != handle.id as usize && !data.ptr.get().is_null() {
                elements.reverse();
                return Ok(PathFromAncestor {
                    ancestor: NodeHandle { id: curr_id as u64 },
                    path: elements,
                });
            }

            elements.push(data.element.clone());
            curr_id = data.parent_id.id as usize;
        }
        elements.reverse();
        Ok(PathFromAncestor { ancestor: NodeHandle::root(), path: elements })
    }

    fn get_path_elements(&self, handle: NodeHandle) -> Result<Vec<PathElement>, Status> {
        Ok(self.get_path_from_ancestor(handle)?.path)
    }

    fn resolve_node(&self, handle: NodeHandle) -> Result<&Value, Status> {
        let current_addr = std::ptr::from_ref(self) as usize;
        let cached_addr = self.registry.owner_addr.get();
        if cached_addr != 0 && cached_addr != current_addr {
            // FastSerdeJson instance has moved in memory; invalidate all cached raw pointers.
            self.registry.invalidate_ptrs();
        }
        self.registry.owner_addr.set(current_addr);

        let handle_to_data = self.registry.handle_to_data.borrow();
        let curr_id = handle.id as usize;
        let data = handle_to_data
            .get(curr_id)
            .ok_or_else(|| invalid_handle_error(format!("Invalid NodeHandle ID: {}", handle.id)))?;

        let cached_ptr = data.ptr.get();
        if !cached_ptr.is_null() {
            // SAFETY: [Why unsafe] Caching raw pointers in `HandleRegistry` avoids repeatedly traversing
            // the AST from the root on every lookup, reducing access time from O(depth) to O(1).
            //
            // Soundness guarantee relies on two core properties:
            // 1. Pointer validity: The cached pointer references a valid, unmodified object in memory because:
            //    - The pointer is only ever assigned from `Value` objects owned by this `FastSerdeJson` instance.
            //    - Since assignment, this instance has not moved in memory; otherwise the relocation is detected
            //      via `owner_addr` and all cached pointers are immediately invalidated.
            //    - This instance is not a clone of another document, as cloning invokes `invalidate_ptrs()`.
            //    - Any operations that mutate `self.value` also invoke `invalidate_ptrs()`, clearing all cached pointers.
            // 2. Aliasing: Obtaining an immutable reference here is sound because:
            //    - `resolve_node` borrows `&'a self` immutably, ensuring no concurrent mutable references exist
            //      to `self.value` or any of its owned child nodes.
            //    - Via lifetime elision, the signature is interpreted as `(&'a self, ...) -> Result<&'a Value, Status>`.
            //      This guarantees the returned reference cannot outlive `&self`, preventing invalid aliasing.
            return Ok(unsafe { &*cached_ptr });
        }
        drop(handle_to_data);

        let path_info = self.get_path_from_ancestor(handle)?;
        let ancestor_node = if path_info.ancestor.id == 0 {
            &self.value
        } else {
            let handle_to_data = self.registry.handle_to_data.borrow();
            let anc_data = &handle_to_data[path_info.ancestor.id as usize];
            let ptr = anc_data.ptr.get();
            if ptr.is_null() {
                &self.value
            } else {
                // SAFETY: [Why unsafe] Caching raw pointers in `HandleRegistry` avoids repeatedly traversing
                // the AST from the root on every lookup, reducing access time from O(depth) to O(1).
                //
                // Soundness guarantee relies on two core properties:
                // 1. Pointer validity: The cached pointer references a valid, unmodified object in memory because:
                //    - The pointer is only ever assigned from `Value` objects owned by this `FastSerdeJson` instance.
                //    - Since assignment, this instance has not moved in memory; otherwise the relocation is detected
                //      via `owner_addr` and all cached pointers are immediately invalidated.
                //    - This instance is not a clone of another document, as cloning invokes `invalidate_ptrs()`.
                //    - Any operations that mutate `self.value` also invoke `invalidate_ptrs()`, clearing all cached pointers.
                // 2. Aliasing: Obtaining an immutable reference here is sound because:
                //    - `resolve_node` borrows `&'a self` immutably, ensuring no concurrent mutable references exist
                //      to `self.value` or any of its owned child nodes.
                //    - Via lifetime elision, the signature is interpreted as `(&'a self, ...) -> Result<&'a Value, Status>`.
                //      This guarantees the returned reference cannot outlive `&self`, preventing invalid aliasing.
                unsafe { &*ptr }
            }
        };

        let curr = traverse_path(ancestor_node, &path_info.path)?;
        let handle_to_data = self.registry.handle_to_data.borrow();
        if let Some(data) = handle_to_data.get(handle.id as usize) {
            data.ptr.set(curr as *const _);
        }
        Ok(curr)
    }

    fn resolve_node_mut(&mut self, handle: NodeHandle) -> Result<&mut Value, Status> {
        self.registry.invalidate_ptrs();
        let path = self.get_path_elements(handle)?;
        traverse_path_mut(&mut self.value, &path)
    }

    pub fn clone_subtree(&self, handle: NodeHandle) -> Result<FastSerdeJson, Status> {
        if handle.id == 0 {
            Ok(self.clone())
        } else {
            let node = self.resolve_node(handle)?;
            Ok(FastSerdeJson::new(node.clone()))
        }
    }

    pub fn try_parse(raw_data: &[u8]) -> Result<FastSerdeJson, Status> {
        match serde_json::from_slice(raw_data) {
            Ok(value) => Ok(Self::new(value)),
            Err(err) => Err(invalid_argument_error(err.to_string())),
        }
    }

    pub fn new_object() -> Self {
        Self::new(serde_json::json!({}))
    }

    pub fn new_array() -> Self {
        Self::new(serde_json::json!([]))
    }

    pub fn from_i64(v: i64) -> Self {
        Self::new(serde_json::json!(v))
    }

    pub fn try_from_f64(v: f64) -> Result<FastSerdeJson, Status> {
        match Number::from_f64(v) {
            Some(n) => Ok(Self::new(Value::Number(n))),
            None => Err(invalid_argument_error(format!("Invalid JSON number: {}", v))),
        }
    }

    pub fn from_bool(v: bool) -> Self {
        Self::new(serde_json::json!(v))
    }

    pub fn new_null() -> Self {
        Self::new(Value::Null)
    }

    pub fn try_from_utf8(raw_value: &[u8]) -> Result<FastSerdeJson, Status> {
        let value = match std::str::from_utf8(raw_value) {
            Ok(data) => data,
            Err(err) => return Err(invalid_argument_error(err.to_string())),
        };
        Ok(Self::new(serde_json::json!(value)))
    }

    pub fn get_field(
        &self,
        raw_field_name: &[u8],
        handle: NodeHandle,
    ) -> Result<NodeHandle, Status> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(invalid_argument_error(err.to_string())),
        };
        let node = self.resolve_node(handle)?;
        match node.get(field_name) {
            Some(child_node) => self.registry.get_or_create_child_handle(
                handle,
                PathElement::Field(field_name.into()),
                child_node as *const _,
            ),
            None => Err(failed_precondition_error(format!(
                "Field '{}' not found in JSON object",
                field_name
            ))),
        }
    }

    pub fn get_field_string(
        &self,
        raw_field_name: &[u8],
        handle: NodeHandle,
    ) -> Result<CppVecU8, Status> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(invalid_argument_error(err.to_string())),
        };
        let node = self.resolve_node(handle)?;
        match node.get(field_name).and_then(|v| v.as_str()) {
            Some(s) => Ok(s.as_bytes().into()),
            None => Err(failed_precondition_error(format!("Field '{}' is not string", field_name))),
        }
    }

    pub fn get_field_bool(
        &self,
        raw_field_name: &[u8],
        handle: NodeHandle,
    ) -> Result<bool, Status> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(invalid_argument_error(err.to_string())),
        };
        let node = self.resolve_node(handle)?;
        match node.get(field_name).and_then(|v| v.as_bool()) {
            Some(b) => Ok(b),
            None => {
                Err(failed_precondition_error(format!("Field '{}' is not boolean", field_name)))
            }
        }
    }

    pub fn get_field_int(&self, raw_field_name: &[u8], handle: NodeHandle) -> Result<i64, Status> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(invalid_argument_error(err.to_string())),
        };
        let node = self.resolve_node(handle)?;
        match node.get(field_name).and_then(|v| v.as_i64()) {
            Some(i) => Ok(i),
            None => {
                Err(failed_precondition_error(format!("Field '{}' is not integer", field_name)))
            }
        }
    }

    pub fn get_field_double(
        &self,
        raw_field_name: &[u8],
        handle: NodeHandle,
    ) -> Result<f64, Status> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(invalid_argument_error(err.to_string())),
        };
        let node = self.resolve_node(handle)?;
        match node.get(field_name).and_then(|v| v.as_f64()) {
            Some(f) => Ok(f),
            None => Err(failed_precondition_error(format!("Field '{}' is not double", field_name))),
        }
    }

    pub fn get_field_object(
        &self,
        raw_field_name: &[u8],
        handle: NodeHandle,
    ) -> Result<NodeHandle, Status> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(invalid_argument_error(err.to_string())),
        };
        let node = self.resolve_node(handle)?;
        match node.get(field_name) {
            Some(child_node @ Value::Object(_)) => self.registry.get_or_create_child_handle(
                handle,
                PathElement::Field(field_name.into()),
                child_node as *const _,
            ),
            _ => Err(failed_precondition_error(format!("Field '{}' is not object", field_name))),
        }
    }

    pub fn get_field_array(
        &self,
        raw_field_name: &[u8],
        handle: NodeHandle,
    ) -> Result<CppVecNodeHandle, Status> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(invalid_argument_error(err.to_string())),
        };
        let node = self.resolve_node(handle)?;
        match node.get(field_name) {
            Some(field_val @ Value::Array(a)) => {
                let field_handle = self.registry.get_or_create_child_handle(
                    handle,
                    PathElement::Field(field_name.into()),
                    field_val as *const _,
                )?;
                let mut handles = Vec::with_capacity(a.len());
                for (i, elem) in a.iter().enumerate() {
                    handles.push(self.registry.get_or_create_child_handle(
                        field_handle,
                        PathElement::Index(i),
                        elem as *const _,
                    )?);
                }
                Ok(handles.into())
            }
            _ => Err(failed_precondition_error(format!("Field '{}' is not array", field_name))),
        }
    }

    pub fn get_field_array_element(
        &self,
        raw_field_name: &[u8],
        index: usize,
        handle: NodeHandle,
    ) -> Result<NodeHandle, Status> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(invalid_argument_error(err.to_string())),
        };
        let node = self.resolve_node(handle)?;
        match node.get(field_name) {
            Some(child_node @ Value::Array(a)) => match a.get(index) {
                Some(elem_node) => {
                    let field_handle = self.registry.get_or_create_child_handle(
                        handle,
                        PathElement::Field(field_name.into()),
                        child_node as *const _,
                    )?;
                    self.registry.get_or_create_child_handle(
                        field_handle,
                        PathElement::Index(index),
                        elem_node as *const _,
                    )
                }
                None => Err(out_of_range_error(format!(
                    "Index {} out of bounds for array field '{}' of length {}",
                    index,
                    field_name,
                    a.len()
                ))),
            },
            _ => Err(failed_precondition_error(format!("Field '{}' is not array", field_name))),
        }
    }

    pub fn get_bool(&self, handle: NodeHandle) -> Result<bool, Status> {
        let node = self.resolve_node(handle)?;
        match node.as_bool() {
            Some(b) => Ok(b),
            None => Err(failed_precondition_error("This object is not boolean")),
        }
    }

    pub fn get_int(&self, handle: NodeHandle) -> Result<i64, Status> {
        let node = self.resolve_node(handle)?;
        match node.as_i64() {
            Some(i) => Ok(i),
            None => Err(failed_precondition_error("This object is not integer")),
        }
    }

    pub fn get_double(&self, handle: NodeHandle) -> Result<f64, Status> {
        let node = self.resolve_node(handle)?;
        match node.as_f64() {
            Some(f) => Ok(f),
            None => Err(failed_precondition_error("This object is not double")),
        }
    }

    pub fn get_string(&self, handle: NodeHandle) -> Result<CppVecU8, Status> {
        let node = self.resolve_node(handle)?;
        match node.as_str() {
            Some(s) => Ok(s.as_bytes().into()),
            None => Err(failed_precondition_error("This object is not string")),
        }
    }

    pub fn get_array(&self, handle: NodeHandle) -> Result<CppVecNodeHandle, Status> {
        let node = self.resolve_node(handle)?;
        match node.as_array() {
            Some(a) => {
                let mut handles = Vec::with_capacity(a.len());
                for (i, elem) in a.iter().enumerate() {
                    handles.push(self.registry.get_or_create_child_handle(
                        handle,
                        PathElement::Index(i),
                        elem as *const _,
                    )?);
                }
                Ok(handles.into())
            }
            None => Err(failed_precondition_error("This object is not array")),
        }
    }

    pub fn get_array_element(
        &self,
        index: usize,
        handle: NodeHandle,
    ) -> Result<NodeHandle, Status> {
        let node = self.resolve_node(handle)?;
        match node.as_array() {
            Some(a) => match a.get(index) {
                Some(child_node) => self.registry.get_or_create_child_handle(
                    handle,
                    PathElement::Index(index),
                    child_node as *const _,
                ),
                None => Err(out_of_range_error(format!(
                    "Index {} out of bounds for array of length {}",
                    index,
                    a.len()
                ))),
            },
            None => Err(failed_precondition_error("This object is not array")),
        }
    }

    pub fn is_empty(&self, handle: NodeHandle) -> Result<bool, Status> {
        let node = self.resolve_node(handle)?;
        let empty = match node {
            Value::Null => true,
            Value::Array(arr) => arr.is_empty(),
            Value::Object(obj) => obj.is_empty(),
            _ => false,
        };
        Ok(empty)
    }

    pub fn is_null(&self, handle: NodeHandle) -> bool {
        self.resolve_node(handle).map(|n| n.is_null()).unwrap_or(false)
    }

    pub fn is_boolean(&self, handle: NodeHandle) -> bool {
        self.resolve_node(handle).map(|n| n.is_boolean()).unwrap_or(false)
    }

    pub fn is_number(&self, handle: NodeHandle) -> bool {
        self.resolve_node(handle).map(|n| n.is_number()).unwrap_or(false)
    }

    pub fn is_i64(&self, handle: NodeHandle) -> bool {
        self.resolve_node(handle).map(|n| n.is_i64()).unwrap_or(false)
    }

    pub fn is_f64(&self, handle: NodeHandle) -> bool {
        self.resolve_node(handle).map(|n| n.is_f64()).unwrap_or(false)
    }

    pub fn is_string(&self, handle: NodeHandle) -> bool {
        self.resolve_node(handle).map(|n| n.is_string()).unwrap_or(false)
    }

    pub fn is_array(&self, handle: NodeHandle) -> bool {
        self.resolve_node(handle).map(|n| n.is_array()).unwrap_or(false)
    }

    pub fn is_object(&self, handle: NodeHandle) -> bool {
        self.resolve_node(handle).map(|n| n.is_object()).unwrap_or(false)
    }

    pub fn to_string(&self, sort_keys: bool, handle: NodeHandle) -> CppVecU8 {
        let node = match self.resolve_node(handle) {
            Ok(n) => n,
            Err(_) => return CppVecU8::new(),
        };
        if sort_keys {
            let mut val = node.clone();
            val.sort_all_objects();
            return val.to_string().into_bytes().into();
        }
        node.to_string().into_bytes().into()
    }

    pub fn keys(&self, handle: NodeHandle) -> Result<CppVecVecU8, Status> {
        let node = self.resolve_node(handle)?;
        let object = match node.as_object() {
            Some(o) => o,
            None => return Err(failed_precondition_error("This value is not an object")),
        };
        let keys: CppVecVecU8 = object.keys().map(|k| k.as_bytes().into()).collect();
        Ok(keys)
    }

    pub fn has_field(&self, raw_field_name: &[u8], handle: NodeHandle) -> Result<bool, Status> {
        let node = self.resolve_node(handle)?;
        match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => Ok(node.get(field_name).is_some()),
            Err(err) => Err(invalid_argument_error(err.to_string())),
        }
    }

    fn add_field_value(
        &mut self,
        raw_field_name: &[u8],
        value: Value,
        handle: NodeHandle,
    ) -> Status {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field) => field,
            Err(err) => return invalid_argument_error(err.to_string()),
        };
        let node = match self.resolve_node_mut(handle) {
            Ok(n) => n,
            Err(err) => return err,
        };
        if let Value::Object(map) = node {
            map.insert(field_name.to_string(), value);
            ok()
        } else {
            internal_error("JSON value is not an object")
        }
    }

    pub fn add_field<T: Serialize>(
        &mut self,
        raw_field_name: &[u8],
        value: T,
        handle: NodeHandle,
    ) -> Status {
        match serde_json::to_value(value) {
            Ok(json_value) => self.add_field_value(raw_field_name, json_value, handle),
            Err(err) => invalid_argument_error(format!("Failed to serialize value: {}", err)),
        }
    }

    pub fn add_field_int(
        &mut self,
        raw_field_name: &[u8],
        value: i64,
        handle: NodeHandle,
    ) -> Status {
        self.add_field_value(raw_field_name, serde_json::json!(value), handle)
    }

    pub fn add_field_bool(
        &mut self,
        raw_field_name: &[u8],
        value: bool,
        handle: NodeHandle,
    ) -> Status {
        self.add_field_value(raw_field_name, serde_json::json!(value), handle)
    }

    pub fn add_field_double(
        &mut self,
        raw_field_name: &[u8],
        value: f64,
        handle: NodeHandle,
    ) -> Status {
        match Number::from_f64(value) {
            Some(n) => self.add_field_value(raw_field_name, Value::Number(n), handle),
            None => invalid_argument_error(format!("Invalid JSON number: {}", value)),
        }
    }

    pub fn add_field_null(&mut self, raw_field_name: &[u8], handle: NodeHandle) -> Status {
        self.add_field_value(raw_field_name, Value::Null, handle)
    }

    pub fn add_field_string(
        &mut self,
        raw_field_name: &[u8],
        value_raw: &[u8],
        handle: NodeHandle,
    ) -> Status {
        let value = match std::str::from_utf8(value_raw) {
            Ok(value) => value,
            Err(err) => return invalid_argument_error(err.to_string()),
        };
        self.add_field_value(raw_field_name, serde_json::json!(value), handle)
    }

    pub fn add_field_object(
        &mut self,
        raw_field_name: &[u8],
        obj: FastSerdeJson,
        handle: NodeHandle,
    ) -> Status {
        self.add_field_value(raw_field_name, obj.value, handle)
    }

    pub fn add_field_array(
        &mut self,
        raw_field_name: &[u8],
        items: CppVecFastSerdeJson,
        handle: NodeHandle,
    ) -> Status {
        let vec_values: Vec<Value> = items.into_iter().map(|item| item.value).collect();
        self.add_field_value(raw_field_name, Value::Array(vec_values), handle)
    }

    pub fn is_json_equal(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
