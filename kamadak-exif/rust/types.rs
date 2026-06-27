// For common types.
// Used for struct Tag in tag.rs
pub type OptionString = Option<VecU8>;

#[macro_export]
macro_rules! make_vec_struct {
    ( $( $type:ty, $vec_name:ident ),* $(,)? ) => {
        $(
            #[repr(C)]
            #[derive(Default, Debug, Clone, PartialEq)]
            pub struct $vec_name(pub Vec<$type>);

            impl $vec_name {
                pub fn new() -> $vec_name {
                    $vec_name(Vec::new())
                }
                pub fn into_vec(self) -> Vec<$type> {
                    self.0
                }
                pub fn as_slice(&self) -> &[$type] {
                    &self.0
                }
                pub fn as_mut_slice(&mut self) -> &mut [$type] {
                    &mut self.0
                }
                pub fn new_repeating(value: $type, len: usize) -> $vec_name {
                    $vec_name(vec![value; len])
                }
                pub fn as_ptr(&self) -> *const $type {
                    self.0.as_ptr()
                }
                pub fn as_mut_ptr(&mut self) -> *mut $type {
                    self.0.as_mut_ptr()
                }
                pub fn len(&self) -> usize {
                    self.0.len()
                }
                pub fn is_empty(&self) -> bool {
                    self.0.is_empty()
                }
                pub fn copy_from_slice(slice: &[$type]) -> $vec_name {
                    $vec_name(slice.to_vec())
                }
            }

            impl From<Vec<$type>> for $vec_name {
                fn from(value: Vec<$type>) -> Self {
                    $vec_name(value)
                }
            }

            impl From<&[$type]> for $vec_name {
                fn from(value: &[$type]) -> Self {
                    $vec_name(value.to_vec())
                }
            }

            impl From<$vec_name> for Vec<$type> {
                fn from(v: $vec_name) -> Self {
                    v.into_vec()
                }
            }
        )*
    };
}

make_vec_struct!(u32, VecU32);
make_vec_struct!(u8, VecU8);
make_vec_struct!(u16, VecU16);
make_vec_struct!(i8, VecI8);
make_vec_struct!(i16, VecI16);
make_vec_struct!(i32, VecI32);
make_vec_struct!(f32, VecF32);
make_vec_struct!(f64, VecF64);

impl VecU8 {
    pub fn as_mut_vec(&mut self) -> &mut Vec<u8> {
        &mut self.0
    }
}

// Nested vector
make_vec_struct!(VecU8, VecVecU8);
impl From<VecVecU8> for Vec<Vec<u8>> {
    fn from(v: VecVecU8) -> Self {
        v.into_vec().into_iter().map(|inner| inner.into_vec()).collect()
    }
}

pub type OptionField = Option<crate::tiff::Field>;

#[repr(C)]
#[derive(Default, Debug, Clone, PartialEq)]
pub struct VecField(Vec<crate::tiff::Field>);

impl VecField {
    pub fn new() -> VecField {
        VecField(Vec::new())
    }
    pub fn into_vec(self) -> Vec<crate::tiff::Field> {
        self.0
    }
    pub fn as_slice(&self) -> &[crate::tiff::Field] {
        &self.0
    }
    pub fn as_mut_slice(&mut self) -> &mut [crate::tiff::Field] {
        &mut self.0
    }
    pub fn as_ptr(&self) -> *const crate::tiff::Field {
        self.0.as_ptr()
    }
    pub fn as_mut_ptr(&mut self) -> *mut crate::tiff::Field {
        self.0.as_mut_ptr()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Vec<crate::tiff::Field>> for VecField {
    fn from(value: Vec<crate::tiff::Field>) -> Self {
        VecField(value)
    }
}

impl From<&[crate::tiff::Field]> for VecField {
    fn from(value: &[crate::tiff::Field]) -> Self {
        VecField(value.to_vec())
    }
}

#[repr(C)]
#[derive(Default, Debug, Clone, PartialEq)]
pub struct TiffExifDataRs {
    pub fields: VecField,
    pub is_little_endian: bool,
}
