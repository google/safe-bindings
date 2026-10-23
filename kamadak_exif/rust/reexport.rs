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

    /// Reinterprets a shared reference to an unsigned-integer [`Value`] (`Byte`, `Short`, or
    /// `Long`) as a shared reference to [`UIntValue`].
    #[inline]
    pub(crate) fn ref_from(v: &Value) -> Result<&Self, Error> {
        match *v {
            Value::Byte(_) | Value::Short(_) | Value::Long(_) => {
                // SAFETY: Creating the shared reference `&*(v as *const Value as *const Self)`
                // requires the target pointer to be non-null, properly aligned for `UIntValue`,
                // dereferenceable for `size_of::<UIntValue>()` bytes, pointing to an initialized
                // valid `UIntValue`, and obeying shared-reference aliasing for the lifetime of `v`
                // per the Rust Reference. Because `v: &Value` is a live shared reference with no
                // intervening mutation and `UIntValue` is a single-field `#[repr(transparent)]`
                // wrapper around `Value` with identical size, alignment, layout, and validity
                // invariants, all obligations hold, and the returned `&UIntValue` borrows `*v`
                // immutably for the lifetime of `v`.
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
