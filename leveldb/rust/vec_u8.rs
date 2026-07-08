#[repr(C)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

impl From<String> for VecU8 {
    fn from(v: String) -> Self {
        Self(v.into_bytes())
    }
}

impl From<&str> for VecU8 {
    fn from(v: &str) -> Self {
        Self(v.as_bytes().to_vec())
    }
}

impl From<&[u8]> for VecU8 {
    fn from(v: &[u8]) -> Self {
        Self(v.to_vec())
    }
}
