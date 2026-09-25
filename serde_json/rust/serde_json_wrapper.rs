//! `serde_json` wrapper for C++.
//!
//! This wraps `serde_json::Value` and provides APIs, that go/crubit can generate a C++ header for.
//! APIs are designed to:
//! - Yield idiomatic C++ code when bridged by Crubit. This means, the value cannot be wrapped on
//!   the C++ side, because C++ cannot transparently wrap a type and cast from/to its inner value,
//!   which is needed to give out references and use the copy constructor.
//! - Mimic the `serde_json::Value` API. We don't want to introduce much custom logic here. We're
//!   not JSON experts and want to expose serde_json mostly as-is.
//!
//! Memory safety: While parsing is memory-safe, usage from C++ cannot guarantee memory safety.
//! Mutating or dropping/deconstructing a `Value` invalidates references to it and its nested
//! values. C++ users must ensure references are not reused after mutation/destruction.

use ffi_11::CStrExt as _;
use ref_cast::RefCast;
use std::ops::{Index, IndexMut};

pub type Status = Result<(), String>;
pub type StatusOr<T> = Result<T, String>;
#[inline(always)]
fn invalid_argument_error(message: impl Into<String>) -> String {
    format!("INVALID_ARGUMENT: {}", message.into())
}
#[inline(always)]
fn not_found_error(message: impl Into<String>) -> String {
    format!("NOT_FOUND: {}", message.into())
}
#[inline(always)]
fn out_of_range_error(message: impl Into<String>) -> String {
    format!("OUT_OF_RANGE: {}", message.into())
}
#[inline(always)]
fn failed_precondition_error(message: impl Into<String>) -> String {
    format!("FAILED_PRECONDITION: {}", message.into())
}
mod json_sanitizer;
mod serde_json_wrapper_ignore_comments;
pub use serde_json_wrapper_ignore_comments::parse_ignore_comments;

/// Parses the given JSON data into a [Value].
#[crubit_annotate::cpp_name("Parse")]
pub fn parse(data: &[u8]) -> StatusOr<Value> {
    match serde_json::from_slice(data) {
        Ok(value) => Ok(Value { value }),
        Err(err) => Err(invalid_argument_error(err.to_string())),
    }
}

type ToStringResult = String;

/// Serializes the given [Value] to a string of JSON.
#[crubit_annotate::cpp_name("ToString")]
pub fn to_string(value: &Value) -> ToStringResult {
    // Serialization can fail if:
    // - the value is not valid JSON, e.g. contains non-string keys or infinite numbers
    //   --> impossible for serde_json::Value, which represents any _valid_ JSON value
    // - the underlying buffer errors on write
    //   --> impossible for `String`
    serde_json::to_string(&value.value)
        .expect("Value is guaranteed to be serializable to JSON")
}

/// Serializes the given [Value] to a pretty-printed string of JSON.
#[crubit_annotate::cpp_name("ToStringPretty")]
pub fn to_string_pretty(value: &Value) -> ToStringResult {
    // Serialization can fail if:
    // - the value is not valid JSON, e.g. contains non-string keys or infinite numbers
    //   --> impossible for serde_json::Value, which represents any _valid_ JSON value
    // - the underlying buffer errors on write
    //   --> impossible for `String`
    serde_json::to_string_pretty(&value.value)
        .expect("Value is guaranteed to be serializable to JSON")
}

/// A JSON value.
///
/// # UTF-8
///
/// This stores strings as UTF-8. All methods working with strings, accept `&[u8]` and do a lossy
/// conversion to a UTF-8 string internally. This avoids panics and makes the common case easy to
/// use from C++. But it could lead to (a) incorrect/unexpected behavior and (b) it incurs an
/// allocation when the given string is not UTF-8.
///
/// If you expect to with non-UTF-8 data, validate it manually before passing it to this library.
#[derive(Default, PartialEq, Clone, RefCast)]
#[repr(transparent)]
pub struct Value {
    value: serde_json::Value,
}

impl Value {
    /// Returns a new [Value] that wraps the given `serde_json::Value`.
    pub fn new(value: serde_json::Value) -> Self {
        Value { value }
    }
}

// Bridge `serde_json::Value` methods.
impl Value {
    /// Returns the bool value of this [Value] if it is a bool.
    pub fn as_bool(&self) -> Option<bool> {
        self.value.as_bool()
    }

    /// Returns the double value of this [Value] if it is a double.
    pub fn as_double(&self) -> Option<f64> {
        self.value.as_f64()
    }

    /// Returns the int64 value of this [Value] if it is a int64.
    pub fn as_int64(&self) -> Option<i64> {
        self.value.as_i64()
    }

    /// Returns the string value of this [Value] if it is a string.
    pub fn as_str(&self) -> Option<&str> {
        self.value.as_str()
    }

    /// Returns the uint64 value of this [Value] if it is a uint64.
    pub fn as_uint64(&self) -> Option<u64> {
        self.value.as_u64()
    }

    /// Returns true if this [Value] is an array.
    pub fn is_array(&self) -> bool {
        self.value.is_array()
    }

    /// Returns true if this [Value] is bool.
    pub fn is_bool(&self) -> bool {
        self.value.is_boolean()
    }

    /// Returns true if this [Value] is a double.
    pub fn is_double(&self) -> bool {
        self.value.is_f64()
    }

    /// Returns true if this [Value] is an int64.
    pub fn is_int64(&self) -> bool {
        self.value.is_i64()
    }

    /// Returns true if this [Value] is null.
    pub fn is_null(&self) -> bool {
        self.value.is_null()
    }

    /// Returns true if this [Value] is number.
    pub fn is_number(&self) -> bool {
        self.value.is_number()
    }

    /// Returns true if this [Value] is object.
    pub fn is_object(&self) -> bool {
        self.value.is_object()
    }

    /// Returns true if this [Value] is string.
    pub fn is_string(&self) -> bool {
        self.value.is_string()
    }

    /// Returns true if this [Value] is a uint64.
    pub fn is_uint64(&self) -> bool {
        self.value.is_u64()
    }

    /// Reorders entries in all nested objects within this [Value] alphabetically.
    ///
    /// This destroys the original source order or insertion order.
    ///
    /// Invalidates references and iterators to items in this or any nested object.
    pub fn sort_all_objects(&mut self) {
        self.value.sort_all_objects();
    }
}

// Add convenience methods modeled after nlohmann/json.
//
// They are partly necessary because Crubit doesn't support the `serde_json::Object` type and partly
// make migration from nlohmann/json easier.
impl Value {
    /// Returns a new [Value] that is an empty array.
    pub fn array() -> Self {
        Value { value: serde_json::Value::Array(Vec::new()) }
    }

    /// Returns a new [Value] that is an empty object.
    pub fn object() -> Self {
        Value { value: serde_json::Value::Object(serde_json::Map::new()) }
    }

    /// Returns true if this [Value] is empty.
    ///
    /// Empty matches `size() == 0` and is defined as:
    /// - `null`
    /// - an empty array `[]`
    /// - an empty object `{}`
    pub fn empty(&self) -> bool {
        match &self.value {
            serde_json::Value::Null => true,
            serde_json::Value::Array(arr) => arr.is_empty(),
            serde_json::Value::Object(obj) => obj.is_empty(),
            _ => false,
        }
    }

    /// Returns the length of this [Value].
    ///
    /// Length matches the iterators and is defined as:
    /// - `null`: 0
    /// - array: length of the array
    /// - object: length of the object
    /// - primitive: 1
    pub fn size(&self) -> usize {
        match &self.value {
            serde_json::Value::Null => 0,
            serde_json::Value::Array(arr) => arr.len(),
            serde_json::Value::Object(obj) => obj.len(),
            _ => 1,
        }
    }

    /// Returns true if this [Value] is an object and contains the given key.
    pub fn contains(&self, key: &[u8]) -> bool {
        self.value
            .as_object()
            .is_some_and(|obj| obj.contains_key(String::from_utf8_lossy(key).as_ref()))
    }

    /// Returns a reference to the [Value] associated with the given key, if this [Value] is an
    /// object and the key exists.
    pub fn find(&self, key: &[u8]) -> Option<&Value> {
        self.value
            .as_object()
            .and_then(|obj| obj.get(String::from_utf8_lossy(key).as_ref()))
            .map(RefCast::ref_cast)
    }

    /// Returns a mutable reference to the [Value] associated with the given key, if this [Value]
    /// is an object and the key exists.
    pub fn find_mut(&mut self, key: &[u8]) -> Option<&mut Value> {
        self.value
            .as_object_mut()
            .and_then(|obj| obj.get_mut(String::from_utf8_lossy(key).as_ref()))
            .map(RefCast::ref_cast_mut)
    }

    /// Removes the element at the given index from this [Value] if it is an array.
    ///
    /// Returns the removed element.
    ///
    /// Invalidates references and iterators of this [Value].
    ///
    /// Returns an error if this [Value] is not an array or if `index >= size()`.
    pub fn erase_at(&mut self, index: usize) -> StatusOr<Value> {
        match &mut self.value {
            serde_json::Value::Array(arr) => {
                if index >= arr.len() {
                    Err(out_of_range_error("Index out of bounds"))
                } else {
                    Ok(Value { value: arr.remove(index) })
                }
            }
            _ => Err(invalid_argument_error("JSON value is not an array"))
        }
    }

    /// Removes the given key from this [Value] if it is an object.
    ///
    /// Returns the removed value if the key was found.
    ///
    /// Invalidates references and iterators of this [Value].
    ///
    /// Returns an error if this [Value] is not an object.
    pub fn erase(&mut self, key: &[u8]) -> StatusOr<Value> {
        match &mut self.value {
            serde_json::Value::Object(obj) => {
                let key = String::from_utf8_lossy(key);
                obj.remove(key.as_ref())
                    .map(|v| Value { value: v })
                    .ok_or_else(|| not_found_error(format!("Key {} not found in object", key)))
            }
            _ => Err(invalid_argument_error("JSON value is not an object"))
        }
    }

    /// Moves all elements from the given [Value] into this [Value].
    ///
    /// If this [Value] is null, it is replaced with an empty value before moving the elements.
    ///
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    ///
    /// Invalidates references and iterators of this [Value].
    ///
    /// Returns an error if the given [Value] is not an array or object or is not the same type as
    /// this value.
    pub fn append(&mut self, mut value: Value) -> Status {
        match (&mut self.value, &mut value.value) {
            (serde_json::Value::Array(arr), serde_json::Value::Array(value_arr)) => {
                arr.append(value_arr);
                Ok(())
            }
            (serde_json::Value::Object(obj), serde_json::Value::Object(value_obj)) => {
                obj.append(value_obj);
                Ok(())
            }
            (serde_json::Value::Null, serde_json::Value::Array(value_arr)) => {
                let _ = std::mem::replace(
                    &mut self.value,
                    serde_json::Value::Array(std::mem::take(value_arr)),
                );
                Ok(())
            }
            (serde_json::Value::Null, serde_json::Value::Object(value_obj)) => {
                let _ = std::mem::replace(
                    &mut self.value,
                    serde_json::Value::Object(std::mem::take(value_obj)),
                );
                Ok(())
            }
            _ => Err(invalid_argument_error(
                "JSON value is not an array or object or is not the same type as this value",
            ))
        }
    }

    /// Inserts an element at the given index in an array.
    ///
    /// Invalidates references and iterators of this [Value].
    ///
    /// Returns an error if this [Value] is not an array or if `index > size()`.
    pub fn insert(&mut self, index: usize, value: Value) -> Status {
        match &mut self.value {
            serde_json::Value::Array(arr) => {
                if index > arr.len() {
                    Err(out_of_range_error("Index out of bounds"))
                } else {
                    arr.insert(index, value.value);
                    Ok(())
                }
            }
            _ => Err(invalid_argument_error("JSON value is not an array"))
        }
    }

    /// Adds an element to the end of an array.
    ///
    /// If this [Value] is null, it is replaced with an array containing the given value.
    ///
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    ///
    /// Invalidates references and iterators of this [Value].
    ///
    /// Returns an error if this [Value] is not an array or null.
    pub fn push_back(&mut self, value: Value) -> Status {
        match &mut self.value {
            serde_json::Value::Array(arr) => {
                arr.push(value.value);
                Ok(())
            }
            serde_json::Value::Null => {
                let _ =
                    std::mem::replace(&mut self.value, serde_json::Value::Array(vec![value.value]));
                Ok(())
            }
            _ => Err(invalid_argument_error("JSON value is not an array"))
        }
    }

    /// Removes the last element from an array.
    ///
    /// Invalidates references and iterators of this [Value].
    ///
    /// Returns an error if this [Value] is not an array.
    pub fn pop_back(&mut self) -> StatusOr<Value> {
        match &mut self.value {
            serde_json::Value::Array(arr) => {
                arr.pop()
                    .map(|v| Value { value: v })
                    .ok_or_else(|| failed_precondition_error("Array is empty"))
            }
            _ => Err(invalid_argument_error("JSON value is not an array"))
        }
    }

    /// Returns an iterator over the key-value pairs of this [Value].
    ///
    /// If this [Value] is not an object, returns an empty iterator.
    pub fn items<'a>(&'a self) -> ItemsIter<'a> {
        match &self.value {
            serde_json::Value::Object(map) => ItemsIter::Object(map.iter()),
            _ => ItemsIter::Null,
        }
    }

    /// Returns a mutable iterator over the key-value pairs of this [Value].
    ///
    /// If this [Value] is not an object, returns an empty iterator.
    pub fn items_mut<'a>(&'a mut self) -> ItemsIterMut<'a> {
        match &mut self.value {
            serde_json::Value::Object(map) => ItemsIterMut::Object(map.iter_mut()),
            _ => ItemsIterMut::Null,
        }
    }
}

macro_rules! impl_from {
    ($t:ty) => {
        impl From<$t> for Value {
            fn from(s: $t) -> Self {
                Self { value: serde_json::Value::from(s) }
            }
        }
    };
}

impl_from!(bool);
impl_from!(f32);
impl_from!(f64);
impl_from!(i8);
impl_from!(i16);
impl_from!(i32);
impl_from!(i64);
impl_from!(u8);
impl_from!(u16);
impl_from!(u32);
impl_from!(u64);

impl From<&[u8]> for Value {
    fn from(s: &[u8]) -> Self {
        Self { value: serde_json::Value::from(String::from_utf8_lossy(s)) }
    }
}

// Allow construction from a C++ string literal.
//
// Using `Value("foo")` otherwise fails from C++ because "foo" is a `const char[4]`, which decays to
// a `const char*` and C++ has a standard conversion from pointer to boolean and standard
// conversions take precedence over user-defined conversions like the one from `const char*` to
// `rs_std::StrRef` that we'd like to use here. The fix is to provide an exactly matching
// constructor for `const char*`.
//
// NOTE(b/562037348): This is unsound. Remove as soon as Crubit offers something better.
impl From<*const ffi_11::c_char> for Value {
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn from(ptr: *const ffi_11::c_char) -> Self {
        if ptr.is_null() {
            return Self { value: serde_json::Value::Null };
        }

        // SAFETY:
        // - Precondition: The C++ caller MUST pass a valid, null-terminated C-string.
        //   This is guaranteed for C++ string literals like "foo".
        // - The `is_null()` check above guarantees we don't pass a null pointer.
        // - The memory is only read synchronously during this function call.
        // We don't really know what's behind this pointer. Normally we need to make this function
        // unsafe, but we cannot make a `From` impl unsafe. This is a trade-off to make the C++
        // usage easier.
        let c_str = unsafe { std::ffi::CStr::from_ffi_11_ptr(ptr) };
        Self { value: serde_json::Value::from(c_str.to_string_lossy()) }
    }
}

impl Index<usize> for Value {
    type Output = Self;

    /// Index into the [Value] if it's an array.
    ///
    /// Panics if this [Value] is not an array or if the index is out of bounds.
    fn index(&self, index: usize) -> &Self {
        Self::ref_cast(self.value.get(index).expect("index within bounds"))
    }
}

impl IndexMut<usize> for Value {
    /// Write into the [Value] if it's an array.
    ///
    /// If `index >= size()`, the array is resized to `index + 1` and filled with nulls.
    ///
    /// If this [Value] is null, it's converted to an empty array and then the index is inserted.
    ///
    /// Panics if this [Value] is neither an array nor null.
    fn index_mut(&mut self, index: usize) -> &mut Self {
        match &mut self.value {
            v @ serde_json::Value::Null => {
                *v = serde_json::Value::Array(vec![serde_json::Value::Null; index + 1]);
                Self::ref_cast_mut(&mut v[index])
            }
            serde_json::Value::Array(arr) => {
                if index >= arr.len() {
                    arr.resize(index + 1, serde_json::Value::Null);
                }
                Self::ref_cast_mut(&mut arr[index])
            }
            v => panic!(
                "cannot access index {:?} of JSON {}",
                index,
                match v {
                    serde_json::Value::Null => "null",
                    serde_json::Value::Bool(_) => "bool",
                    serde_json::Value::Number(_) => "number",
                    serde_json::Value::String(_) => "string",
                    serde_json::Value::Array(_) => "array",
                    serde_json::Value::Object(_) => "object",
                }
            ),
        }
    }
}

impl Index<&[u8]> for Value {
    type Output = Self;

    /// Index into the [Value] if it's an object.
    ///
    /// Panics if this [Value] is not an object or if the key does not exist in the object.
    fn index(&self, index: &[u8]) -> &Self {
        Self::ref_cast(self.value.get(String::from_utf8_lossy(index).as_ref()).expect("key exists"))
    }
}

impl IndexMut<&[u8]> for Value {
    /// Write into the [Value] if it's an object.
    ///
    /// If the key does not exist, the value is inserted into the object.
    ///
    /// If this [Value] is null, it's converted to an empty object and then the key is inserted.
    ///
    /// Panics if this [Value] is neither an object nor null.
    fn index_mut(&mut self, index: &[u8]) -> &mut Self {
        Self::ref_cast_mut(&mut self.value[String::from_utf8_lossy(index).as_ref()])
    }
}

impl<'a> IntoIterator for &'a Value {
    type Item = &'a Value;
    type IntoIter = Iter<'a>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        match &self.value {
            serde_json::Value::Null => Iter::Null,
            serde_json::Value::Array(arr) => Iter::Array(arr.iter()),
            serde_json::Value::Object(map) => Iter::Object(map.values()),
            v => Iter::Primitive(std::iter::once(v)),
        }
    }
}

impl<'a> IntoIterator for &'a mut Value {
    type Item = &'a mut Value;
    type IntoIter = IterMut<'a>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        match &mut self.value {
            serde_json::Value::Null => IterMut::Null,
            serde_json::Value::Array(arr) => IterMut::Array(arr.iter_mut()),
            serde_json::Value::Object(map) => IterMut::Object(map.values_mut()),
            v => IterMut::Primitive(std::iter::once(v)),
        }
    }
}

#[derive(Clone)]
pub enum Iter<'a> {
    Null,
    Primitive(std::iter::Once<&'a serde_json::Value>),
    Array(std::slice::Iter<'a, serde_json::Value>),
    Object(serde_json::map::Values<'a>),
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Value;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Iter::Null => None,
            Iter::Primitive(iter) => iter.next().map(Value::ref_cast),
            Iter::Array(iter) => iter.next().map(Value::ref_cast),
            Iter::Object(iter) => iter.next().map(Value::ref_cast),
        }
    }
}

pub enum IterMut<'a> {
    Null,
    Primitive(std::iter::Once<&'a mut serde_json::Value>),
    Array(std::slice::IterMut<'a, serde_json::Value>),
    Object(serde_json::map::ValuesMut<'a>),
}

impl<'a> Iterator for IterMut<'a> {
    type Item = &'a mut Value;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            IterMut::Null => None,
            IterMut::Primitive(iter) => iter.next().map(Value::ref_cast_mut),
            IterMut::Array(iter) => iter.next().map(Value::ref_cast_mut),
            IterMut::Object(iter) => iter.next().map(Value::ref_cast_mut),
        }
    }
}

#[derive(Clone)]
pub struct Item<'a> {
    pub key: &'a str,
    pub value: &'a Value,
}

#[derive(Clone)]
pub enum ItemsIter<'a> {
    Null,
    Object(serde_json::map::Iter<'a>),
}

impl<'a> Iterator for ItemsIter<'a> {
    type Item = Item<'a>;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            ItemsIter::Null => None,
            ItemsIter::Object(iter) => {
                iter.next().map(|(k, v)| Item { key: k.as_str(), value: Value::ref_cast(v) })
            }
        }
    }
}

pub struct ItemMut<'a> {
    pub key: &'a str,
    pub value: &'a mut Value,
}

pub enum ItemsIterMut<'a> {
    Null,
    Object(serde_json::map::IterMut<'a>),
}

impl<'a> Iterator for ItemsIterMut<'a> {
    type Item = ItemMut<'a>;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            ItemsIterMut::Null => None,
            ItemsIterMut::Object(iter) => {
                iter.next().map(|(k, v)| ItemMut { key: k.as_str(), value: Value::ref_cast_mut(v) })
            }
        }
    }
}
