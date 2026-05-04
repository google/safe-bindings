google3::import! {
  "//third_party/absl/status:status";
  "//third_party/absl/status:status_wrapper";
  "//third_party/rust/anyhow/v1:anyhow";
  "//third_party/rust/serde/v1:serde";
  "//third_party/rust/serde_json/v1:serde_json";
}

use crate::make_result_type;
use crate::make_vec_type;
use crate::raw_string::RawString;
use anyhow::anyhow;
use serde::Serialize;
use status_wrapper::StatusWrapper;

#[derive(Default, PartialEq, Clone)]
pub struct SerdeJson {
    pub(crate) value: serde_json::Value,
}

impl std::fmt::Debug for SerdeJson {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "SerdeJson")
    }
}

impl From<&Vec<serde_json::Value>> for VecSerdeJson {
    fn from(value: &Vec<serde_json::Value>) -> Self {
        value.iter().map(|v| SerdeJson { value: v.clone() }).collect::<Vec<SerdeJson>>().into()
    }
}

impl SerdeJson {
    /// Creates a new [SerdeJson] from provided string buffer `raw_data`.
    pub fn parse_string(raw_data: cc_std::std::string_view) -> ResultSerdeJson {
        let data = match raw_data.to_str() {
            Ok(data) => data,
            Err(err) => return Err(anyhow::Error::new(err)).into(),
        };

        match serde_json::from_str(data) {
            Ok(value) => Ok(Self { value }).into(),
            Err(err) => Err(anyhow!(err)).into(),
        }
    }

    /// Creates a new [SerdeJson] of type object
    pub fn create_object() -> Self {
        SerdeJson { value: serde_json::json!({}) }
    }

    /// Creates a new [SerdeJson] of type i64
    pub fn create_int(v: i64) -> Self {
        SerdeJson { value: serde_json::json!(v) }
    }

    /// Creates a new [SerdeJson] of type double
    /// Returns error if value is NaN or infinity.
    pub fn create_double(v: f64) -> ResultSerdeJson {
        match serde_json::Number::from_f64(v) {
            Some(n) => Ok(SerdeJson { value: serde_json::Value::Number(n) }).into(),
            None => Err(anyhow::anyhow!("Invalid JSON number: {}", v)).into(),
        }
    }

    /// Creates a new [SerdeJson] of type bool
    pub fn create_bool(v: bool) -> Self {
        SerdeJson { value: serde_json::json!(v) }
    }

    /// Creates a new [SerdeJson] of type null
    pub fn create_null() -> Self {
        SerdeJson { value: serde_json::Value::Null }
    }

    /// Creates a new [SerdeJson] of type string
    pub fn create_string(raw_value: cc_std::std::string_view) -> ResultSerdeJson {
        let value = match raw_value.to_str() {
            Ok(data) => data,
            Err(err) => return Err(anyhow::Error::new(err)).into(),
        };
        SerdeJson { value: serde_json::json!(value) }.into()
    }

    /// Returns a new [SerdeJson] for a given field name in the JSON.
    /// The type of the underlying data is unknown, and has to be tested using
    /// is_{bool,string,int,double} functions, and value can be obtained
    /// using get_{bool,string,int,double} functions.
    pub fn get_field(&self, raw_field_name: cc_std::std::string_view) -> ResultSerdeJson {
        let field_name = match raw_field_name.to_str() {
            Ok(field_name) => field_name,
            Err(err) => return Err(anyhow::Error::new(err)).into(),
        };
        match self.value.get(field_name) {
            Some(value) => Ok(Self { value: value.clone() }).into(),
            None => Err(anyhow::anyhow!("Field '{}' not found in JSON object", field_name)).into(),
        }
    }

    /// Returns a string for a given field name in the JSON.
    pub fn get_field_string(&self, raw_field_name: cc_std::std::string_view) -> ResultString {
        let field_name = match raw_field_name.to_str() {
            Ok(field_name) => field_name,
            Err(err) => return Err(anyhow::Error::new(err)).into(),
        };
        match self.value[field_name].as_str() {
            Some(s) => cc_std::std::string::from(s.to_string()).into(),
            None => Err(anyhow::anyhow!("Field '{}' is not string", field_name)).into(),
        }
    }

    /// Returns a boolean for a given field name in the JSON.
    pub fn get_field_bool(&self, raw_field_name: cc_std::std::string_view) -> ResultBool {
        let field_name = match raw_field_name.to_str() {
            Ok(field_name) => field_name,
            Err(err) => return Err(anyhow::Error::new(err)).into(),
        };
        match self.value[field_name].as_bool() {
            Some(b) => b.into(),
            None => Err(anyhow::anyhow!("Field '{}' is not boolean", field_name)).into(),
        }
    }

    /// Returns a int for a given field name in the JSON.
    pub fn get_field_int(&self, raw_field_name: cc_std::std::string_view) -> Resulti64 {
        let field_name = match raw_field_name.to_str() {
            Ok(field_name) => field_name,
            Err(err) => return Err(anyhow::Error::new(err)).into(),
        };
        match self.value[field_name].as_i64() {
            Some(i) => i.into(),
            None => Err(anyhow::anyhow!("Field '{}' is not integer", field_name)).into(),
        }
    }

    /// Returns a JSON object (represented by the [SerdeJson]) for a given field name in
    /// the JSON.
    pub fn get_field_object(&self, raw_field_name: cc_std::std::string_view) -> ResultSerdeJson {
        let field_name = match raw_field_name.to_str() {
            Ok(field_name) => field_name,
            Err(err) => return Err(anyhow::Error::new(err)).into(),
        };
        match &self.value[field_name] {
            o @ serde_json::Value::Object(_) => Ok(Self { value: o.clone() }).into(),
            _ => Err(anyhow::anyhow!("Field '{}' is not object", field_name)).into(),
        }
    }

    /// Returns a double for a given field name in the JSON.
    pub fn get_field_double(&self, raw_field_name: cc_std::std::string_view) -> ResultDouble {
        let field_name = match raw_field_name.to_str() {
            Ok(field_name) => field_name,
            Err(err) => return Err(anyhow::Error::new(err)).into(),
        };
        match self.value[field_name].as_f64() {
            Some(f) => f.into(),
            None => Err(anyhow::anyhow!("Field '{}' is not double", field_name)).into(),
        }
    }

    /// Returns an array of [SerdeJson] for a given field name in the JSON.
    pub fn get_field_array(&self, raw_field_name: cc_std::std::string_view) -> ResultVecSerdeJson {
        let field_name = match raw_field_name.to_str() {
            Ok(field_name) => field_name,
            Err(err) => return Err(anyhow::Error::new(err)).into(),
        };
        match self.value[field_name].as_array() {
            Some(a) => std::convert::Into::<VecSerdeJson>::into(a).into(),
            None => Err(anyhow::anyhow!("Field '{}' is not array", field_name)).into(),
        }
    }

    /// Returns the boolean value of this [SerdeJson] if it is a boolean.
    pub fn get_bool(&self) -> ResultBool {
        match self.value.as_bool() {
            Some(b) => b.into(),
            None => Err(anyhow::anyhow!("This object is not boolean")).into(),
        }
    }

    /// Returns the int value of this [SerdeJson] if it is a int.
    pub fn get_int(&self) -> Resulti64 {
        match self.value.as_i64() {
            Some(i) => i.into(),
            None => Err(anyhow::anyhow!("This object is not integer")).into(),
        }
    }

    /// Returns the double value of this [SerdeJson] if it is a double.
    pub fn get_double(&self) -> ResultDouble {
        match self.value.as_f64() {
            Some(f) => f.into(),
            None => Err(anyhow::anyhow!("This object is not double")).into(),
        }
    }

    /// Returns the string value of this [SerdeJson] if it is a string.
    pub fn get_string(&self) -> ResultString {
        match self.value.as_str() {
            Some(s) => cc_std::std::string::from(s.to_string()).into(),
            None => Err(anyhow::anyhow!("This object is not string")).into(),
        }
    }

    /// Returns the array of [SerdeJson] if the object is an array.
    pub fn get_array(&self) -> ResultVecSerdeJson {
        match self.value.as_array() {
            Some(a) => std::convert::Into::<VecSerdeJson>::into(a).into(),
            None => Err(anyhow::anyhow!("This object is not array")).into(),
        }
    }

    /// Returns true if this [SerdeJson] is null.
    pub fn is_null(&self) -> bool {
        self.value.is_null()
    }

    /// Returns true if this [SerdeJson] is boolean.
    pub fn is_boolean(&self) -> bool {
        self.value.is_boolean()
    }

    /// Returns true if this [SerdeJson] is number.
    pub fn is_number(&self) -> bool {
        self.value.is_number()
    }

    /// Returns true if this [SerdeJson] is i64.
    pub fn is_i64(&self) -> bool {
        self.value.is_i64()
    }

    /// Returns true if this [SerdeJson] is f64.
    pub fn is_f64(&self) -> bool {
        self.value.is_f64()
    }

    /// Returns true if this [SerdeJson] is string.
    pub fn is_string(&self) -> bool {
        self.value.is_string()
    }

    /// Returns true if this [SerdeJson] is array.
    pub fn is_array(&self) -> bool {
        self.value.is_array()
    }

    /// Returns true if this [SerdeJson] is object.
    pub fn is_object(&self) -> bool {
        self.value.is_object()
    }

    /// Converts a [SerdeJson] to a string.
    pub fn to_string(&self) -> cc_std::std::string {
        self.value.to_string().into()
    }

    pub fn get_keys(&self) -> ResultVecRawString {
        let object = match self.value.as_object() {
            Some(o) => o,
            None => return Err(anyhow::anyhow!("This isn't a object")).into(),
        };

        Ok(object.keys().map(|k| k.as_str().into()).collect::<Vec<RawString>>().into()).into()
    }

    /// Returns true if the JSON object contains the given field.
    pub fn has_field(&self, raw_field_name: cc_std::std::string_view) -> ResultBool {
        raw_field_name.to_str().map_or_else(
            |err| Err(anyhow::Error::new(err)).into(),
            |field_name| Ok(self.value.get(field_name).is_some()).into(),
        )
    }

    /// Adds a field to the JSON object.
    pub fn add_field<T: Serialize>(
        &mut self,
        raw_field_name: cc_std::std::string_view,
        value: T,
    ) -> StatusWrapper {
        let field_name = match raw_field_name.to_str() {
            Ok(field) => field,
            Err(err) => return status::invalid_argument(err.to_string()).into(),
        };

        if let serde_json::Value::Object(map) = &mut self.value {
            match serde_json::to_value(value) {
                Ok(json_value) => {
                    map.insert(field_name.to_string(), json_value);
                    status::OkStatus().into()
                }
                Err(err) => {
                    status::invalid_argument(format!("Failed to serialize value: {}", err)).into()
                }
            }
        } else {
            status::internal("JSON value is not an object").into()
        }
    }

    /// Adds an integer field to the JSON object.
    pub fn add_field_int(
        &mut self,
        raw_field_name: cc_std::std::string_view,
        value: i64,
    ) -> StatusWrapper {
        self.add_field(raw_field_name, value)
    }

    /// Adds a boolean field to the JSON object.
    pub fn add_field_bool(
        &mut self,
        raw_field_name: cc_std::std::string_view,
        value: bool,
    ) -> StatusWrapper {
        self.add_field(raw_field_name, value)
    }

    /// Adds a double field to the JSON object.
    pub fn add_field_double(
        &mut self,
        raw_field_name: cc_std::std::string_view,
        value: f64,
    ) -> StatusWrapper {
        self.add_field(raw_field_name, value)
    }

    /// Adds a null field to the JSON object.
    pub fn add_field_null(&mut self, raw_field_name: cc_std::std::string_view) -> StatusWrapper {
        self.add_field(raw_field_name, serde_json::Value::Null)
    }

    /// Adds a string field to the JSON object.
    pub fn add_field_string(
        &mut self,
        raw_field_name: cc_std::std::string_view,
        value_raw: cc_std::std::string_view,
    ) -> StatusWrapper {
        let value = match value_raw.to_str() {
            Ok(value) => value,
            Err(err) => return status::invalid_argument(err.to_string()).into(),
        };
        self.add_field(raw_field_name, value)
    }

    /// Adds an object field to the JSON object.
    pub fn add_field_object(
        &mut self,
        raw_field_name: cc_std::std::string_view,
        obj: SerdeJson,
    ) -> StatusWrapper {
        self.add_field(raw_field_name, obj.value)
    }

    /// Adds an array field to the JSON object.
    pub fn add_field_array(
        &mut self,
        raw_field_name: cc_std::std::string_view,
        items: &[SerdeJson],
    ) -> StatusWrapper {
        self.add_field(
            raw_field_name,
            items.iter().map(|v| v.value.clone()).collect::<Vec<serde_json::Value>>(),
        )
    }
}

// NOTE: b/367916605 - Remove make_result_type and make_vec_type macros.
make_result_type!(SerdeJson);
make_result_type!(cc_std::std::string, ResultString);
make_result_type!(bool, ResultBool);
make_result_type!(i64, Resulti64);
make_result_type!(f64, ResultDouble);

make_vec_type!(RawString, VecRawString);
make_result_type!(VecRawString, ResultVecRawString);

make_vec_type!(SerdeJson, VecSerdeJson);
make_result_type!(VecSerdeJson, ResultVecSerdeJson);
