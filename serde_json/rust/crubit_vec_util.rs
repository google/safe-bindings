//! NOTE: b/399861833 - This is a duplicate of google3/security/xml/rust/internal/crubit_vec_util.rs
//! and google3/security/ise_memory_safety/safe_xml_parsing/bindings/rust/crubit_vec_util.rs.
//!
//! This file provides macros for creating Crubit-compatible `Vec` type.
//! This has been adapted from the VecU8 type in //base/rust/rust_vec_u8.rs.

google3::import! {
  "//third_party/rust/paste/v1:paste";
}

/// Macro for generating a Crubit-compatible `Vec` type. The `Vec` type
/// has similar methods as a Rust `Vec` and converts to/from the `Vec` type.
/// This is adapted from VecU8 (//base/rust/rust_vec_u8.rs).
///
/// # Prerequisites
/// The element type needs to implement `Debug`, to be able to print the vector,
/// `Clone` to implement to_vec, and `PartialEq` and `PartialOrd` to be an element
/// of the vector.
///
/// # Example
/// ```rust
/// struct Foo {
///    a: u32,
/// }
/// make_vec_type!(Foo); // Generates a `VecFoo` that can be bridged to C.
/// // Alternatively define your own name for the Vec type.
/// make_vec_type!(u32, VecU32);
/// ...
/// let vec_foo = vec![Foo { a: 32 }].into();
/// let vec_u32 = vec![32].into();
/// ...
/// ```
#[macro_export]
macro_rules! make_vec_type {
    ($type:ty, $vec_name:ident) => {
        #[repr(C)]
        #[derive(Default, Debug, Clone, PartialEq)]
        pub struct $vec_name(Vec<$type>);

        impl $vec_name {
            #[doc="See [std::vec::Vec::new](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.new)."]
            pub fn new() -> $vec_name {
                $vec_name(Vec::new())
            }

            #[doc="Converts into the underlying `Vec<$type>`.
                   NOTE: We could also implement Into<vec<$type>> but opted
                   for keeping this as close to VecU8 as possible."]
            pub fn into_vec(self) -> Vec<$type> {
                self.0
            }

            #[doc="Views this as a slice of `$type`."]
            pub fn as_slice(&self) -> &[$type] {
                &self.0
            }

            #[doc="Views this as a mutable slice of `$type`."]
            pub fn as_mut_slice(&mut self) -> &[$type] {
                &mut self.0
            }

            #[doc="See [std::vec](https://doc.rust-lang.org/std/macro.vec.html)."]
            pub fn new_repeating(value: $type, len: usize) -> $vec_name {
                $vec_name(vec![value; len])
            }

            #[doc="See [std::vec::Vec::as_ptr](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.as_ptr)."]
            pub fn as_ptr(&self) -> *const $type {
                self.0.as_ptr()
            }

            #[doc="See [std::vec::Vec::as_mut_ptr](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.as_mut_ptr)."]
            pub fn as_mut_ptr(&mut self) -> *mut $type {
                self.0.as_mut_ptr()
            }

            #[doc="See [std::vec::Vec::len](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.len)."]
            pub fn len(&self) -> usize {
                self.0.len()
            }

            #[doc="See [std::vec::Vec::is_empty](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.is_empty)."]
            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
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
    };
    ($vec_name:ident) => {
        paste::paste! {
          make_vec_type!($vec_name, [<Vec$vec_name>]);
        }
    };
}

#[cfg(test)]
mod test {
    google3::import! {
        "//third_party/gtest_rust/googletest";
    }

    use googletest::prelude::*;

    #[derive(Debug, PartialEq, Clone, Copy, Default)]
    pub struct Demo {
        val: u32,
    }

    make_vec_type!(Demo);
    make_vec_type!(u32, VecU32);

    #[gtest]
    fn test_vec_new() {
        // Testing that `new` and `new_repeating` are working correctly.
        let elem = Demo { val: 3 };

        let v = VecDemo::new();
        expect_that!(v.0, elements_are![]);
        let v = VecU32::new();
        expect_that!(v.0, elements_are![]);

        let v = VecDemo::new_repeating(elem, 0);
        expect_that!(v.0, elements_are![]);
        let v = VecU32::new_repeating(3, 0);
        expect_that!(v.0, elements_are![]);

        let v = VecDemo::new_repeating(elem, 5);
        expect_that!(v.0, container_eq([elem, elem, elem, elem, elem]));
        let v = VecU32::new_repeating(3, 5);
        expect_that!(v.0, container_eq([3, 3, 3, 3, 3]));
    }

    #[gtest]
    fn test_vec_from_vec() {
        // Testing that `from` works correctly.
        let cd1 = Demo { val: 1 };
        let cd2 = Demo { val: 2 };
        let cd3 = Demo { val: 3 };

        let src: Vec<Demo> = vec![cd1, cd2, cd3];
        let v = VecDemo::from(src);
        expect_that!(v.0, container_eq([cd1, cd2, cd3]));
        let src: Vec<u32> = vec![1, 2, 3];
        let v = VecU32::from(src);
        expect_that!(v.0, container_eq([1, 2, 3]));

        let src: &[Demo] = &[cd1, cd2, cd3];
        let v = VecDemo::from(src);
        expect_that!(v.0, container_eq([cd1, cd2, cd3]));
        let src: &[u32] = &[1, 2, 3];
        let v = VecU32::from(src);
        expect_that!(v.0, container_eq([1, 2, 3]));
    }

    #[gtest]
    fn test_vec_into_vec() {
        // Testing that `into_vec`, `as_slice` and `as_mut_slice` are working
        // correctly.
        let cd1 = Demo { val: 1 };
        let cd2 = Demo { val: 2 };
        let cd3 = Demo { val: 3 };

        let v: VecDemo = vec![cd1, cd2, cd3].into();
        expect_that!(v.into_vec(), container_eq([cd1, cd2, cd3]));
        let v: VecU32 = vec![1, 2, 3].into();
        expect_that!(v.into_vec(), container_eq([1, 2, 3]));

        let v: VecDemo = vec![cd1, cd2, cd3].into();
        expect_that!(v.as_slice(), container_eq([cd1, cd2, cd3]));
        let v: VecU32 = vec![1, 2, 3].into();
        expect_that!(v.as_slice(), container_eq([1, 2, 3]));

        let mut v: VecDemo = vec![cd1, cd2, cd3].into();
        expect_that!(v.as_mut_slice(), container_eq([cd1, cd2, cd3]));
        let mut v: VecU32 = vec![1, 2, 3].into();
        expect_that!(v.as_mut_slice(), container_eq([1, 2, 3]));
    }

    #[gtest]
    fn test_vec_other_methods() {
        let num_source: Vec<u32> = vec![10, 20, 30, 40, 50];
        let demo_source: Vec<Demo> =
            num_source.clone().into_iter().map(|num| Demo { val: num }).collect();

        let mut v_num: VecU32 = num_source.clone().into();
        let mut v_demo: VecDemo = demo_source.clone().into();

        expect_eq!(v_num.len(), num_source.len());
        expect_eq!(v_demo.len(), demo_source.len());

        expect_false!(v_num.is_empty());
        expect_false!(v_demo.is_empty());

        expect_that!(v_num.as_ptr(), not(eq(std::ptr::null())));
        expect_that!(v_demo.as_ptr(), not(eq(std::ptr::null())));

        expect_that!(v_num.as_mut_ptr(), not(eq(std::ptr::null_mut())));
        expect_that!(v_demo.as_mut_ptr(), not(eq(std::ptr::null_mut())));
    }
}
