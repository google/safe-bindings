use crate::make_vec_type;
use crate::raw_string::RawString;
use serde::Serialize;
use std::sync::Arc;
// NOTE: b/517030085 - Crubit doesn't seem to support () here, so using a u8 for now.
pub type Status = Result<u8, RawString>;

#[inline(always)]
fn ok() -> Status {
    Ok(0)
}

#[inline(always)]
fn internal_error(message: impl Into<String>) -> Status {
    Err(message.into().into())
}

#[inline(always)]
fn invalid_argument_error(message: impl Into<String>) -> Status {
    Err(message.into().into())
}

#[derive(Default, PartialEq, Clone, Debug)]
pub struct GetArrayElementError {
    pub msg: RawString,
    pub is_out_of_bounds: bool,
}

impl GetArrayElementError {
    pub fn new_out_of_bounds(msg: impl Into<String>) -> Self {
        Self { msg: msg.into().into(), is_out_of_bounds: true }
    }

    pub fn new_failed_precondition(msg: impl Into<String>) -> Self {
        Self { msg: msg.into().into(), is_out_of_bounds: false }
    }
}

#[derive(Clone)]
pub struct SerdeJson {
    pub(crate) root: Arc<serde_json::Value>,
    pub(crate) node: *const serde_json::Value,
}

// SAFETY: `SerdeJson` contains a raw pointer (`node`) which makes it `!Send` and `!Sync` by
// default. However, `node` always points into `root` (`Arc<serde_json::Value>`), which keeps the
// target memory valid and immutable for the lifetime of this struct. Since `serde_json::Value` and
// `Arc<T>` are both `Send` and `Sync`, and mutation is strictly guarded by `Arc::make_mut` on the
// root node, sending or sharing `SerdeJson` across threads cannot cause data races or memory
// corruption.
unsafe impl Send for SerdeJson {}
// SAFETY: See `Send` safety justification above; immutable concurrent access to `node` guarded by
// `Arc` is thread-safe.
unsafe impl Sync for SerdeJson {}

impl Default for SerdeJson {
    fn default() -> Self {
        Self::create_null()
    }
}

impl PartialEq for SerdeJson {
    fn eq(&self, other: &Self) -> bool {
        self.as_value() == other.as_value()
    }
}

impl std::fmt::Debug for SerdeJson {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "SerdeJson({:?})", self.as_value())
    }
}

impl From<&Vec<serde_json::Value>> for VecSerdeJson {
    fn from(value: &Vec<serde_json::Value>) -> Self {
        value.iter().map(|v| SerdeJson::from_value(v.clone())).collect::<Vec<SerdeJson>>().into()
    }
}

impl SerdeJson {
    fn from_value(value: serde_json::Value) -> Self {
        let root = Arc::new(value);
        let node = Arc::as_ptr(&root);
        Self { root, node }
    }

    fn as_sub_node(&self, sub_node: &serde_json::Value) -> Self {
        Self { root: self.root.clone(), node: sub_node as *const serde_json::Value }
    }

    fn as_value(&self) -> &serde_json::Value {
        // SAFETY: `self.node` is guaranteed to be non-null, properly aligned, and pointing to a
        // valid `serde_json::Value` owned by `self.root`. The memory is kept alive by `self.root`'s
        // `Arc` reference count for at least the lifetime of `&self`. Furthermore, no mutable
        // aliasing can occur because mutating methods require `&mut self` and use `Arc::make_mut`
        // which clones on shared ownership), preserving immutability for existing views.
        unsafe { &*self.node }
    }

    fn as_mut_value(&mut self) -> Result<&mut serde_json::Value, Status> {
        if std::ptr::eq(self.node, Arc::as_ptr(&self.root)) {
            let root_mut = Arc::make_mut(&mut self.root);
            let ptr = root_mut as *mut serde_json::Value;
            self.node = ptr;
            Ok(root_mut)
        } else {
            Err(internal_error("Cannot mutate a sub-node view of a shared SerdeJson object"))
        }
    }

    /// Creates a new [SerdeJson] from provided string buffer `raw_data`.
    pub fn parse_string(raw_data: &[u8]) -> Result<SerdeJson, RawString> {
        let data = match std::str::from_utf8(raw_data) {
            Ok(data) => data,
            Err(err) => return Err(err.to_string().into()),
        };

        match serde_json::from_str(data) {
            Ok(value) => Ok(Self::from_value(value)),
            Err(err) => Err(err.to_string().into()),
        }
    }

    /// Creates a new [SerdeJson] of type object
    pub fn create_object() -> Self {
        Self::from_value(serde_json::json!({}))
    }

    /// Creates a new [SerdeJson] of type i64
    pub fn create_int(v: i64) -> Self {
        Self::from_value(serde_json::json!(v))
    }

    /// Creates a new [SerdeJson] of type double
    /// Returns error if value is NaN or infinity.
    pub fn create_double(v: f64) -> Result<SerdeJson, RawString> {
        match serde_json::Number::from_f64(v) {
            Some(n) => Ok(Self::from_value(serde_json::Value::Number(n))),
            None => Err(format!("Invalid JSON number: {}", v).into()),
        }
    }

    /// Creates a new [SerdeJson] of type bool
    pub fn create_bool(v: bool) -> Self {
        Self::from_value(serde_json::json!(v))
    }

    /// Creates a new [SerdeJson] of type null
    pub fn create_null() -> Self {
        Self::from_value(serde_json::Value::Null)
    }

    /// Creates a new [SerdeJson] of type string
    pub fn create_string(raw_value: &[u8]) -> Result<SerdeJson, RawString> {
        let value = match std::str::from_utf8(raw_value) {
            Ok(data) => data,
            Err(err) => return Err(err.to_string().into()),
        };
        Ok(Self::from_value(serde_json::json!(value)))
    }

    /// Returns a new [SerdeJson] for a given field name in the JSON.
    /// The type of the underlying data is unknown, and has to be tested using
    /// is_{bool,string,int,double} functions, and value can be obtained
    /// using get_{bool,string,int,double} functions.
    pub fn get_field(&self, raw_field_name: &[u8]) -> Result<SerdeJson, RawString> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(err.to_string().into()),
        };
        match self.as_value().get(field_name) {
            Some(value) => Ok(self.as_sub_node(value)),
            None => Err(format!("Field '{}' not found in JSON object", field_name).into()),
        }
    }

    /// Returns a string for a given field name in the JSON.
    pub fn get_field_string(&self, raw_field_name: &[u8]) -> Result<RawString, RawString> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(err.to_string().into()),
        };
        match self.as_value()[field_name].as_str() {
            Some(s) => Ok(s.into()),
            None => Err(format!("Field '{}' is not string", field_name).into()),
        }
    }

    /// Returns a boolean for a given field name in the JSON.
    pub fn get_field_bool(&self, raw_field_name: &[u8]) -> Result<bool, RawString> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(err.to_string().into()),
        };
        match self.as_value()[field_name].as_bool() {
            Some(b) => Ok(b),
            None => Err(format!("Field '{}' is not boolean", field_name).into()),
        }
    }

    /// Returns a int for a given field name in the JSON.
    pub fn get_field_int(&self, raw_field_name: &[u8]) -> Result<i64, RawString> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(err.to_string().into()),
        };
        match self.as_value()[field_name].as_i64() {
            Some(i) => Ok(i),
            None => Err(format!("Field '{}' is not integer", field_name).into()),
        }
    }

    /// Returns a JSON object (represented by the [SerdeJson]) for a given field name in
    /// the JSON.
    pub fn get_field_object(&self, raw_field_name: &[u8]) -> Result<SerdeJson, RawString> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(err.to_string().into()),
        };
        match &self.as_value()[field_name] {
            o @ serde_json::Value::Object(_) => Ok(self.as_sub_node(o)),
            _ => Err(format!("Field '{}' is not object", field_name).into()),
        }
    }

    /// Returns a double for a given field name in the JSON.
    pub fn get_field_double(&self, raw_field_name: &[u8]) -> Result<f64, RawString> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(err.to_string().into()),
        };
        match self.as_value()[field_name].as_f64() {
            Some(f) => Ok(f),
            None => Err(format!("Field '{}' is not double", field_name).into()),
        }
    }

    /// Returns an array of [SerdeJson] for a given field name in the JSON.
    pub fn get_field_array(&self, raw_field_name: &[u8]) -> Result<VecSerdeJson, RawString> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(err.to_string().into()),
        };
        match self.as_value()[field_name].as_array() {
            Some(a) => Ok(a.iter().map(|v| self.as_sub_node(v)).collect::<Vec<SerdeJson>>().into()),
            None => Err(format!("Field '{}' is not array", field_name).into()),
        }
    }

    /// Returns an element of an array for a given field name in the JSON.
    pub fn get_field_array_element(
        &self,
        raw_field_name: &[u8],
        index: usize,
    ) -> Result<SerdeJson, GetArrayElementError> {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => field_name,
            Err(err) => return Err(GetArrayElementError::new_failed_precondition(err.to_string())),
        };
        match self.as_value()[field_name].as_array() {
            Some(a) => match a.get(index) {
                Some(v) => Ok(self.as_sub_node(v)),
                None => Err(GetArrayElementError::new_out_of_bounds(format!(
                    "Index {} out of bounds for array field '{}' of length {}",
                    index,
                    field_name,
                    a.len()
                ))),
            },
            None => Err(GetArrayElementError::new_failed_precondition(format!(
                "Field '{}' is not array",
                field_name
            ))),
        }
    }

    /// Returns the boolean value of this [SerdeJson] if it is a boolean.
    pub fn get_bool(&self) -> Result<bool, RawString> {
        match self.as_value().as_bool() {
            Some(b) => Ok(b),
            None => Err("This object is not boolean".into()),
        }
    }

    /// Returns the int value of this [SerdeJson] if it is a int.
    pub fn get_int(&self) -> Result<i64, RawString> {
        match self.as_value().as_i64() {
            Some(i) => Ok(i),
            None => Err("This object is not integer".into()),
        }
    }

    /// Returns the double value of this [SerdeJson] if it is a double.
    pub fn get_double(&self) -> Result<f64, RawString> {
        match self.as_value().as_f64() {
            Some(f) => Ok(f),
            None => Err("This object is not double".into()),
        }
    }

    /// Returns the string value of this [SerdeJson] if it is a string.
    pub fn get_string(&self) -> Result<RawString, RawString> {
        match self.as_value().as_str() {
            Some(s) => Ok(s.into()),
            None => Err("This object is not string".into()),
        }
    }

    /// Returns the array of [SerdeJson] if the object is an array.
    pub fn get_array(&self) -> Result<VecSerdeJson, RawString> {
        match self.as_value().as_array() {
            Some(a) => Ok(a.iter().map(|v| self.as_sub_node(v)).collect::<Vec<SerdeJson>>().into()),
            None => Err("This object is not array".into()),
        }
    }

    /// Returns the element of an array at index `index` if the object is an array.
    pub fn get_array_element(&self, index: usize) -> Result<SerdeJson, GetArrayElementError> {
        match self.as_value().as_array() {
            Some(a) => match a.get(index) {
                Some(v) => Ok(self.as_sub_node(v)),
                None => Err(GetArrayElementError::new_out_of_bounds(format!(
                    "Index {} out of bounds for array of length {}",
                    index,
                    a.len()
                ))),
            },
            None => Err(GetArrayElementError::new_failed_precondition("This object is not array")),
        }
    }

    /// Returns true if this [SerdeJson] is null.
    pub fn is_null(&self) -> bool {
        self.as_value().is_null()
    }

    /// Returns true if this [SerdeJson] is boolean.
    pub fn is_boolean(&self) -> bool {
        self.as_value().is_boolean()
    }

    /// Returns true if this [SerdeJson] is number.
    pub fn is_number(&self) -> bool {
        self.as_value().is_number()
    }

    /// Returns true if this [SerdeJson] is i64.
    pub fn is_i64(&self) -> bool {
        self.as_value().is_i64()
    }

    /// Returns true if this [SerdeJson] is f64.
    pub fn is_f64(&self) -> bool {
        self.as_value().is_f64()
    }

    /// Returns true if this [SerdeJson] is string.
    pub fn is_string(&self) -> bool {
        self.as_value().is_string()
    }

    /// Returns true if this [SerdeJson] is array.
    pub fn is_array(&self) -> bool {
        self.as_value().is_array()
    }

    /// Returns true if this [SerdeJson] is object.
    pub fn is_object(&self) -> bool {
        self.as_value().is_object()
    }

    /// Converts a [SerdeJson] to a string.
    pub fn to_string(&self, sort_keys: bool) -> RawString {
        if sort_keys {
            let mut value = self.as_value().clone();
            value.sort_all_objects();
            return value.to_string().into();
        }
        self.as_value().to_string().into()
    }

    pub fn get_keys(&self) -> Result<VecRawString, RawString> {
        let object = match self.as_value().as_object() {
            Some(o) => o,
            None => return Err("This isn't a object".into()),
        };

        Ok(object.keys().map(|k| k.as_str().into()).collect::<Vec<RawString>>().into())
    }

    /// Returns true if the JSON object contains the given field.
    pub fn has_field(&self, raw_field_name: &[u8]) -> Result<bool, RawString> {
        match std::str::from_utf8(raw_field_name) {
            Ok(field_name) => Ok(self.as_value().get(field_name).is_some()),
            Err(err) => Err(err.to_string().into()),
        }
    }

    /// Adds a field to the JSON object.
    pub fn add_field<T: Serialize>(&mut self, raw_field_name: &[u8], value: T) -> Status {
        let field_name = match std::str::from_utf8(raw_field_name) {
            Ok(field) => field,
            Err(err) => return invalid_argument_error(err.to_string()),
        };

        match self.as_mut_value() {
            Ok(serde_json::Value::Object(map)) => match serde_json::to_value(value) {
                Ok(json_value) => {
                    map.insert(field_name.to_string(), json_value);
                    ok()
                }
                Err(err) => invalid_argument_error(format!("Failed to serialize value: {}", err)),
            },
            Ok(_) => internal_error("JSON value is not an object"),
            Err(status) => status,
        }
    }

    /// Adds an integer field to the JSON object.
    pub fn add_field_int(&mut self, raw_field_name: &[u8], value: i64) -> Status {
        self.add_field(raw_field_name, value)
    }

    /// Adds a boolean field to the JSON object.
    pub fn add_field_bool(&mut self, raw_field_name: &[u8], value: bool) -> Status {
        self.add_field(raw_field_name, value)
    }

    /// Adds a double field to the JSON object.
    pub fn add_field_double(&mut self, raw_field_name: &[u8], value: f64) -> Status {
        self.add_field(raw_field_name, value)
    }

    /// Adds a null field to the JSON object.
    pub fn add_field_null(&mut self, raw_field_name: &[u8]) -> Status {
        self.add_field(raw_field_name, serde_json::Value::Null)
    }

    /// Adds a string field to the JSON object.
    pub fn add_field_string(&mut self, raw_field_name: &[u8], value_raw: &[u8]) -> Status {
        let value = match std::str::from_utf8(value_raw) {
            Ok(value) => value,
            Err(err) => return invalid_argument_error(err.to_string()),
        };
        self.add_field(raw_field_name, value)
    }

    /// Adds an object field to the JSON object.
    pub fn add_field_object(&mut self, raw_field_name: &[u8], obj: SerdeJson) -> Status {
        self.add_field(raw_field_name, obj.as_value().clone())
    }

    /// Adds an array field to the JSON object.
    pub fn add_field_array(&mut self, raw_field_name: &[u8], items: &[SerdeJson]) -> Status {
        self.add_field(
            raw_field_name,
            items.iter().map(|v| v.as_value().clone()).collect::<Vec<serde_json::Value>>(),
        )
    }

    /// Compares two SerdeJson objects.
    pub fn is_json_equal(&self, other: &Self) -> bool {
        self.as_value() == other.as_value()
    }
}

make_vec_type!(RawString, VecRawString);
make_vec_type!(SerdeJson, VecSerdeJson);
