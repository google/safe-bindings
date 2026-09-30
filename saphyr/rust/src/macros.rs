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

        // NOTE(b/566272152): Remove this temporary workaround and restore the
        // commented-out `match` implementation once the `rustc` `extern "C"`
        // `bool` return ABI bug is fixed:
        //
        // Why this workaround works:
        // 1. In `-c opt`, LLVM folds a multi-variant check over the 7-variant
        //    `YamlOwned` enum (`Sequence`, `Mapping`, `None`) into a bit-shift
        //    lookup table (`0x33 >> tag` in `%al`) truncated to `i1` (see
        //    http://b/565885681#comment3).
        // 2. `rustc` emits `zeroext` on `bool` (`i1`) returns for the Rust ABI
        //    (`FnAbi::adjust_for_rust_abi`), masking bits 1..7 of `%al`, but
        //    omits `zeroext` on `extern "C"` functions such as Crubit's FFI
        //    thunks (see http://b/566272152#comment1 and
        //    http://b/565885681#comment7).
        // 3. Without `#[inline(never)]`, LLVM inlines `is_scalar` (even when delegating to
        //    single-variant checks `is_defined`, `is_sequence`, and `is_mapping`) into Crubit's
        //    `extern "C"` thunk and re-forms the unmasked `0x33 >> tag` shift inside the `extern
        //    "C"` body, returning `0x0c` for `Sequence` and `0x06` for `Mapping` to C++. Marking
        //    `is_scalar` `#[inline(never)]` keeps it as a standalone Rust-ABI call (`zeroext i1`)
        //    and delegates to single-variant helpers, ensuring `%al` is zero-extended to `0` or `1`
        //    before the FFI thunk returns to C++.
        //
        // Guarded under `-c opt` (`yaml_saphyr.opt`) by
        // `SaphyrBindingsTest.IsScalar*` / `SaphyrBindingsScalarTest.*`
        // (`saphyr_bindings_test.cc`) and `YamlWrapperTest.IsScalar*` /
        // `YamlWrapperScalarTest.*` (`yaml_wrapper_test.cc`).
        //
        // Original implementation to restore:
        // match self.inner() {
        //     Some(YamlOwned::Sequence(_)) | Some(YamlOwned::Mapping(_)) | None => false,
        //     _ => true,
        // }
        #[inline(never)]
        pub fn is_scalar(&self) -> bool {
            self.is_defined() && !self.is_sequence() && !self.is_mapping()
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
