// Copy over some private structs that Crubit cannot generate bindings for
// but is necessary for other public structs/funcs.
use crate::error::Error;
use exif::{Error as KamadakError, Value};
use std::fmt;

// value.rs
/// Helper struct for printing a value in a tag-specific format.
#[derive(Copy, Clone)]
pub struct Display<'a> {
    pub fmt: fn(&mut dyn fmt::Write, &Value) -> fmt::Result,
    pub value: &'a Value,
}

impl<'a> fmt::Display for Display<'a> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (self.fmt)(f, self.value)
    }
}

impl Display<'_> {
    /// Returns the display as a VecU8.
    ///
    /// Since Crubit does not auto generate to_string() for io::Display trait,
    /// we implement this function manually.
    pub fn to_string(&self) -> crate::types::VecU8 {
        crate::types::VecU8(<Self as ToString>::to_string(self).into_bytes())
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct UIntValue(Value);

impl UIntValue {
    pub fn new(v: Value) -> Self {
        Self(v)
    }

    /// SAFETY:
    /// UIntValue is a #[repr(transparent)] wrapper of Value, so the casting is safe here.
    #[inline]
    pub(crate) fn ref_from(v: &Value) -> Result<&Self, Error> {
        match *v {
            Value::Byte(_) | Value::Short(_) | Value::Long(_) => {
                Ok(unsafe { &*(v as *const Value as *const Self) })
            }
            _ => Err(Error::from(KamadakError::UnexpectedValue("Not unsigned integer"))),
        }
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<u32> {
        match self.0 {
            Value::Byte(ref v) => v.get(index).map(|&x| x.into()),
            Value::Short(ref v) => v.get(index).map(|&x| x.into()),
            Value::Long(ref v) => v.get(index).copied(),
            _ => panic!(),
        }
    }
}

impl Default for UIntValue {
    fn default() -> Self {
        Self::new(Value::Byte(Vec::<u8>::new()))
    }
}
