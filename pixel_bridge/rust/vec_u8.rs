/// Minimal wrapper around `Vec<u8>` to allow passing a `Vec<u8>` from Rust to C++.
#[repr(C)]
#[derive(Default, Debug, Clone, PartialEq, PartialOrd)]
pub struct VecU8(Vec<u8>);

impl VecU8 {
    pub fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Vec<u8>> for VecU8 {
    fn from(val: Vec<u8>) -> Self {
        Self(val)
    }
}

impl From<String> for VecU8 {
    fn from(val: String) -> Self {
        Self(val.into_bytes())
    }
}

impl From<&str> for VecU8 {
    fn from(val: &str) -> Self {
        Self(val.as_bytes().to_vec())
    }
}
