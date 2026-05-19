// For common types.
use crate::{make_option_type, make_vec_type};

// Used for struct Tag in tag.rs
make_option_type!(cc_std::std::string, OptionString, Clone);

// Used for struct Value in value.rs
// This macro defines the Vec wrapper and the From implementation simultaneously
macro_rules! make_vec_with_from {
    ( $( $type:ty, $vec_name:ident ),* $(,)? ) => {
        $(
            make_vec_type!($type, $vec_name);
            impl From<$vec_name> for Vec<$type> {
                fn from(v: $vec_name) -> Self {
                    v.into_vec()
                }
            }
        )*
    };
}
make_vec_with_from!(u32, VecU32);
make_vec_with_from!(u8, VecU8);
make_vec_with_from!(u16, VecU16);
make_vec_with_from!(i8, VecI8);
make_vec_with_from!(i16, VecI16);
make_vec_with_from!(i32, VecI32);
make_vec_with_from!(f32, VecF32);
make_vec_with_from!(f64, VecF64);

impl VecU8 {
    pub fn as_mut_vec(&mut self) -> &mut Vec<u8> {
        &mut self.0
    }
}

// Nested vector
make_vec_type!(VecU8);
impl From<VecVecU8> for Vec<Vec<u8>> {
    fn from(v: VecVecU8) -> Self {
        v.into_vec().into_iter().map(|inner| inner.into_vec()).collect()
    }
}
