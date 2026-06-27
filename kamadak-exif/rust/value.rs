use crate::error::Error;
use crate::make_vec_struct;
use crate::reexport::{Display, UIntValue};
use crate::types::{VecF32, VecF64, VecI16, VecI32, VecI8, VecU16, VecU32, VecU8, VecVecU8};
use exif::{
    Rational as KamadakRational, SRational as KamadakSRational, Tag, Value as KamadakValue,
};
use std::fmt;

#[derive(Copy, Clone, Debug)]
pub struct Rational(KamadakRational);

impl Rational {
    /// Creates a new Rational value with the given numerator and denominator.
    pub fn new(num: u32, denom: u32) -> Self {
        Self(KamadakRational { num, denom })
    }

    /// Returns the numerator of the Rational value.
    pub fn num(&self) -> u32 {
        self.0.num
    }

    /// Returns the denominator of the Rational value.
    pub fn denom(&self) -> u32 {
        self.0.denom
    }

    /// Converts the value to an f32.
    pub fn to_f32(&self) -> f32 {
        self.0.to_f32()
    }

    /// Converts the value to an f64.
    pub fn to_f64(&self) -> f64 {
        self.0.to_f64()
    }

    /// Compare two KamadakRational objects.
    pub(crate) fn value_eq(val1: &KamadakRational, val2: &KamadakRational) -> bool {
        u64::from(val1.num) * u64::from(val2.denom) == u64::from(val2.num) * u64::from(val1.denom)
    }
}

impl From<(u32, u32)> for Rational {
    fn from(t: (u32, u32)) -> Self {
        Self(KamadakRational::from(t))
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl PartialEq for Rational {
    fn eq(&self, other: &Rational) -> bool {
        Self::value_eq(&self.0, &other.0)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct SRational(KamadakSRational);

impl SRational {
    /// Creates a new SRational value with the given numerator and denominator.
    pub fn new(num: i32, denom: i32) -> Self {
        Self(KamadakSRational { num, denom })
    }

    /// Returns the numerator of the SRational value.
    pub fn num(&self) -> i32 {
        self.0.num
    }

    /// Returns the denominator of the SRational value.
    pub fn denom(&self) -> i32 {
        self.0.denom
    }

    /// Converts the value to an f32.
    pub fn to_f32(&self) -> f32 {
        self.0.to_f32()
    }

    /// Converts the value to an f64.
    pub fn to_f64(&self) -> f64 {
        self.0.to_f64()
    }

    /// Compare two KamadakSRational objects.
    pub(crate) fn value_eq(val1: &KamadakSRational, val2: &KamadakSRational) -> bool {
        i64::from(val1.num) * i64::from(val2.denom) == i64::from(val2.num) * i64::from(val1.denom)
    }
}

impl From<(i32, i32)> for SRational {
    fn from(t: (i32, i32)) -> Self {
        Self(KamadakSRational::from(t))
    }
}

impl fmt::Display for SRational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl PartialEq for SRational {
    fn eq(&self, other: &SRational) -> bool {
        Self::value_eq(&self.0, &other.0)
    }
}

make_vec_struct!(Rational, VecRational);
make_vec_struct!(SRational, VecSRational);

impl From<VecRational> for Vec<KamadakRational> {
    fn from(v: VecRational) -> Self {
        v.into_vec().into_iter().map(|x: Rational| x.0).collect()
    }
}

impl From<VecSRational> for Vec<KamadakSRational> {
    fn from(v: VecSRational) -> Self {
        v.into_vec().into_iter().map(|x: SRational| x.0).collect()
    }
}

/// This macro generates the constructor for each variant of Value.
/// It takes a list of arguments, where each argument is a struct with the following fields:
///
/// * `name`: The name of the variant.
/// * `arg_name`: The name of the argument.
/// * `type`: The type of the argument.
macro_rules! export_value_constructors {
    // Match a list of: VariantName(arg_name: ArgType, ...)
    ( $( $variant:ident ( $( $arg:ident : $type:ty ),* ) ),* $(,)? ) => {
        $(
            /// Create Value variant with the corresponding name.
            #[allow(non_snake_case)]
            pub fn $variant( $( $arg : $type ),* ) -> Self {
                Self(KamadakValue::$variant( $( $arg.into() ),* ))
            }
        )*
    };
}

/// A type and values of a TIFF/Exif field.
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct Value(pub(crate) KamadakValue);

impl Value {
    export_value_constructors!(
        Byte(data: VecU8),
        Ascii(data: VecVecU8),
        Short(data: VecU16),
        Long(data: VecU32),
        Rational(data: VecRational),
        SByte(data: VecI8),
        Undefined(data: VecU8, offset: u32),
        SShort(data: VecI16),
        SLong(data: VecI32),
        SRational(data: VecSRational),
        Float(data: VecF32),
        Double(data: VecF64),
        Unknown(type_id: u16, count: u32, offset: u32),
    );

    /// Returns an object that implements `std::fmt::Display` for
    /// printing a value in a tag-specific format.
    /// The tag of the value is specified as the argument.
    ///
    /// If you want to display with the unit, use `Field::display_value`.
    pub fn display_as(&self, tag: Tag) -> Display {
        // Display is a private struct in the original crate, and therefore Crubit cannot generate
        // binding for it. We copy its implementation into reexport.rs and use it here.
        let result = self.0.display_as(tag);
        Display { fmt: result.fmt, value: result.value }
    }

    /// Returns a VecU32  over the unsigned integers (BYTE, SHORT, or LONG).
    /// `None` is returned if the value is not an unsigned integer type.
    pub fn iter_uint(&self) -> Option<VecU32> {
        // Original function returns Option<UIntIter>
        // which fails with error
        //
        // Error formatting function return type `std::option::Option<value::UIntIter<'__anon1>>`:
        // Failed to get canonical name for DefId(0:499 ~ exif[6643]::value::UIntIter)
        //
        // possibly because UIntIter is also not public. We copied it over to reexport.rs.
        // http://cs///depot/google3/third_party/rust/kamadak_exif/v0_6/src/value.rs;l=222
        // For simplicity, we make it return a VecU32 which is guaranteed to be Crubit compatible.
        self.0.iter_uint().map(|iterator| iterator.collect::<Vec<u32>>().into())
    }

    /// Returns `UIntValue` if the value type is unsigned integer (BYTE,
    /// SHORT, or LONG).  Otherwise `exif::Error` is returned.
    ///
    /// The integer(s) can be obtained by `get(&self, index: usize)` method
    /// on `UIntValue`, which returns `Option<u32>`.
    /// `None` is returned if the index is out of bounds.
    pub fn as_uint(&self) -> Result<UIntValue, Error> {
        match UIntValue::ref_from(&self.0) {
            Ok(value) => Ok(value.clone()),
            Err(error) => Err(error),
        }
    }

    /// Returns the unsigned integer at the given position.
    /// None is returned if the value type is not unsigned integer
    /// (BYTE, SHORT, or LONG) or the position is out of bounds.
    pub fn get_uint(&self, index: usize) -> Option<u32> {
        self.0.get_uint(index)
    }

    /// Returns the integer at the given position, supporting both signed and unsigned types.
    pub fn get_int(&self, index: usize) -> Option<i64> {
        match self.0 {
            KamadakValue::Byte(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::Short(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::Long(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::SByte(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::SShort(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::SLong(ref v) => v.get(index).map(|&x| x.into()),
            _ => None,
        }
    }

    /// Returns the float at the given position, supporting all numeric types (including rationals).
    pub fn get_float(&self, index: usize) -> Option<f32> {
        match self.0 {
            KamadakValue::Byte(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::Short(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::Long(ref v) => v.get(index).map(|&x| x as f32),
            KamadakValue::SByte(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::SShort(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::SLong(ref v) => v.get(index).map(|&x| x as f32),
            KamadakValue::Rational(ref v) => v.get(index).map(|&x| x.to_f32()),
            KamadakValue::SRational(ref v) => v.get(index).map(|&x| x.to_f32()),
            KamadakValue::Float(ref v) => v.get(index).copied(),
            KamadakValue::Double(ref v) => v.get(index).map(|&x| x as f32),
            _ => None,
        }
    }

    /// Returns the double at the given position, supporting all numeric types (including rationals).
    pub fn get_double(&self, index: usize) -> Option<f64> {
        match self.0 {
            KamadakValue::Byte(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::Short(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::Long(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::SByte(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::SShort(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::SLong(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::Rational(ref v) => v.get(index).map(|&x| x.to_f64()),
            KamadakValue::SRational(ref v) => v.get(index).map(|&x| x.to_f64()),
            KamadakValue::Float(ref v) => v.get(index).map(|&x| x.into()),
            KamadakValue::Double(ref v) => v.get(index).copied(),
            _ => None,
        }
    }

    /// Returns the bytes for byte, ASCII, or undefined tags.
    pub fn get_bytes(&self) -> Option<VecU8> {
        match &self.0 {
            KamadakValue::Byte(v) => Some(VecU8::from(v.clone())),
            KamadakValue::Ascii(v) => {
                let mut bytes = Vec::new();
                for (i, s) in v.iter().enumerate() {
                    if i > 0 {
                        bytes.push(0);
                    }
                    bytes.extend_from_slice(s);
                }
                Some(VecU8::from(bytes))
            }
            KamadakValue::Undefined(v, _) => Some(VecU8::from(v.clone())),
            _ => None,
        }
    }

    /// Compare two exif::Value objects.
    pub(crate) fn value_eq(val1: &KamadakValue, val2: &KamadakValue) -> bool {
        // We use this to implement PartialEq for Value.
        // We need this to implement VecValue (wrapper for Vec<Value>)
        match (val1, val2) {
            // Simple Vector variants (Vec implements PartialEq automatically)
            (KamadakValue::Byte(a), KamadakValue::Byte(b)) => a == b,
            (KamadakValue::Ascii(a), KamadakValue::Ascii(b)) => a == b,
            (KamadakValue::Short(a), KamadakValue::Short(b)) => a == b,
            (KamadakValue::Long(a), KamadakValue::Long(b)) => a == b,
            (KamadakValue::SByte(a), KamadakValue::SByte(b)) => a == b,
            (KamadakValue::SShort(a), KamadakValue::SShort(b)) => a == b,
            (KamadakValue::SLong(a), KamadakValue::SLong(b)) => a == b,

            // Rational variants: Use comparison function defined by the wrapper of Rational.
            (KamadakValue::Rational(a), KamadakValue::Rational(b)) => {
                if a.len() != b.len() {
                    return false;
                }
                a.iter().zip(b.iter()).all(|(x, y)| Rational::value_eq(x, y))
            }
            (KamadakValue::SRational(a), KamadakValue::SRational(b)) => {
                if a.len() != b.len() {
                    return false;
                }
                a.iter().zip(b.iter()).all(|(x, y)| SRational::value_eq(x, y))
            }

            // Floating point variants: Vec<f32/f64> implements PartialEq.
            (KamadakValue::Float(a), KamadakValue::Float(b)) => a == b,
            (KamadakValue::Double(a), KamadakValue::Double(b)) => a == b,

            // Complex variants with multiple fields
            // Default to simple comparison.
            (
                KamadakValue::Undefined(vec_a, offset_a),
                KamadakValue::Undefined(vec_b, offset_b),
            ) => vec_a == vec_b && offset_a == offset_b,
            (
                KamadakValue::Unknown(type_a, count_a, offset_a),
                KamadakValue::Unknown(type_b, count_b, offset_b),
            ) => type_a == type_b && count_a == count_b && offset_a == offset_b,
            _ => false,
        }
    }

    /// Compare two exif::Value objects.
    pub fn equals(&self, other: &Value) -> bool {
        Self::value_eq(&self.0, &other.0)
    }
}

impl From<KamadakValue> for Value {
    fn from(value: KamadakValue) -> Self {
        Self(value)
    }
}

impl From<Value> for KamadakValue {
    fn from(value: Value) -> Self {
        value.0
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        Self::value_eq(&self.0, &other.0)
    }
}
