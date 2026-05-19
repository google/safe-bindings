macro_rules! yaml_node_common_impl {
    ($type:ty) => {
        pub fn is_sequence(&self) -> bool {
            self.inner().is_some_and(|n| n.is_sequence())
        }

        pub fn is_mapping(&self) -> bool {
            self.inner().is_some_and(|n| n.is_mapping())
        }

        pub fn is_defined(&self) -> bool {
            self.inner().is_some()
        }

        pub fn is_scalar(&self) -> bool {
            match self.inner() {
                Some(YamlOwned::Sequence(_)) | Some(YamlOwned::Mapping(_)) | None => false,
                _ => true,
            }
        }

        pub fn is_empty(&self) -> bool {
            self.len() == 0
        }

        pub fn len(&self) -> usize {
            match self.inner() {
                Some(YamlOwned::Sequence(seq)) => seq.len(),
                Some(YamlOwned::Mapping(map)) => map.len(),
                _ => 0,
            }
        }

        pub fn as_i64(&self) -> Option<i64> {
            self.inner().and_then(|n| n.as_integer())
        }

        pub fn as_f64(&self) -> Option<f64> {
            self.inner().and_then(|n| n.as_floating_point())
        }

        pub fn as_bool(&self) -> Option<bool> {
            self.inner().and_then(|n| n.as_bool())
        }

        pub fn as_str(&self) -> Option<&str> {
            self.inner().and_then(|n| n.as_str())
        }
    };
}
