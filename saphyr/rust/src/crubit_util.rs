// NOTE - b/399861833: Promote these helper macros for Result<T> to a shared
// library (e.g., //security/rust/crubit_util) to avoid duplication across
// security projects and then remove this file.

//! This file provides macros for creating Crubit-compatible `Result`.

/// Macro for generating a Crubit-compatible `Result` type. The `Result` type
/// has similar methods as a Rust `Result` and converts to/from the underlying
/// type `T` and an `anyhow::Result<T>`. In the error case it returns a human
/// readable string describing the error.
///
/// # Prerequisites
/// The type to wrap needs to implement `Debug` and `Default`, to be able to be
/// printed in an error message and be movable in C++.
///
/// # Example
/// ```rust
/// struct Foo {
///    a: u32,
/// }
/// make_result_type!(Foo); // Generates a `ResultFoo` that can be bridged to C.
/// // Alternatively define your own name for the Result type.
/// make_result_type!(u32, ResultU32);
/// ...
/// let result_foo = foo.into();
/// ...
/// ```
#[macro_export]
macro_rules! make_result_type {
        ($type:ty, $result_name:ident $( < $lt:lifetime > )?) => {
        pub struct $result_name $( <$lt> )? {
            inner: anyhow::Result<$type>,
        }

        impl $( <$lt> )? $result_name $( <$lt> )? {
            pub fn from_ok(ok: $type) -> Self {
                Self { inner: Ok(ok) }
            }

            pub fn from_err(err: cc_std::std::string) -> Self {
                let err_str = String::from_utf8_lossy(err.as_slice()).into_owned();
                Self { inner: Err(anyhow::anyhow!(err_str)) }
            }

            pub fn is_ok(&self) -> bool {
                self.inner.is_ok()
            }

            pub fn is_err(&self) -> bool {
                self.inner.is_err()
            }

            pub fn unwrap(self) -> $type {
                self.inner.unwrap()
            }

            pub fn unwrap_or(self, default: $type) -> $type {
                self.inner.unwrap_or(default)
            }

            pub fn unwrap_err(self) -> cc_std::std::string {
                cc_std::std::string::from(self.inner.unwrap_err().to_string())
            }
        }

        impl $( <$lt> )? From<anyhow::Result<$type>> for $result_name $( <$lt> )? {
            fn from(val: anyhow::Result<$type>) -> Self {
                Self { inner: val }
            }
        }

        impl $( <$lt> )? From<$type> for $result_name $( <$lt> )? {
            fn from(val: $type) -> Self {
                Self { inner: Ok(val) }
            }
        }

        // This default implementation only exists to make the resulting Rust type movable.
        // For more information, see go/crubit/rust/movable_types.
        impl $( <$lt> )? std::default::Default for $result_name $( <$lt> )?
        where
            $type: std::default::Default,
        {
            fn default() -> Self {
                Self { inner: Ok(<$type>::default()) }
            }
        }
    };
    ($result_name:ident) => {
        paste::paste! {
          make_result_type!($result_name, [<Result$result_name>]);
        }
    };
}
