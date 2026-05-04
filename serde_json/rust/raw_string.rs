/// Struct that replaces `String` with `VecU8`
/// NOTE: b/400396686 - Change once String is supported by Crubit.
use crate::make_vec_type;

make_vec_type!(u8, RawString);

impl From<&str> for RawString {
    fn from(item: &str) -> Self {
        item.as_bytes().into()
    }
}
