//! This file provides macro for creating Crubit-compatible `Result` type.
//! This has been copied from //security/audio/chord_bridge/crubit_util.rs.
//NOTE: b/399861833 - Remove file and use centralized dependency.
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
        #[derive(Debug)]
        pub struct $result_name $( <$lt> )? {
            pub(crate) inner: anyhow::Result<$type>,
        }

        impl $( <$lt> )? $result_name $( <$lt> )? {
            pub fn from_ok(ok: $type) -> Self {
                Self { inner: Ok(ok) }
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

            pub fn unwrap_as_ref(&self) -> &$type {
                self.inner.as_ref().unwrap()
            }

            pub fn unwrap_or(self, default: $type) -> $type {
                self.inner.unwrap_or(default)
            }

            pub fn unwrap_err(self) -> String {
                self.inner.unwrap_err().to_string()
            }

            pub fn unwrap_err_ref(&self) -> String {
                self.inner.as_ref().unwrap_err().to_string()
            }
        }

        impl $( <$lt> )? From<anyhow::Result<$type>> for $result_name $( <$lt> )? {
            fn from(val: anyhow::Result<$type>) -> Self {
                Self { inner: val }
            }
        }

        impl $( <$lt> )? From<anyhow::Error> for $result_name $( <$lt> )? {
            fn from(val: anyhow::Error) -> Self {
                Self { inner: Err(val) }
            }
        }

        impl $( <$lt> )? From<$type> for $result_name $( <$lt> )? {
            fn from(val: $type) -> Self {
                Self { inner: Ok(val) }
            }
        }

        #[doc = "This default implementation only exists to make the resulting Rust type movable."]
        #[doc = "For more information, see go/crubit/rust/movable_types."]
        impl $( <$lt> )? std::default::Default for $result_name $( <$lt> )?
        //where
        //    $type: std::default::Default,
        {
            fn default() -> Self {
                Self { inner: Err(anyhow::anyhow!("Use of default-constructed Crubit result")) } //Ok(<$type>::default()) }
            }
        }
    };
    ($result_name:ident $( < $lt:lifetime > )?) => {
        paste::paste! {
          make_result_type!($result_name $( <$lt> )?, [<Result$result_name>] $( <$lt> )?);
        }
    };
}

/// Macro for generating a Crubit-compatible `Option` type. The `Option` type
/// has similar methods as a Rust `Option` and converts to/from the underlying
/// type `T` and an `Option<T>`. It's similar to the `make_result_type!` macro
/// provided by this module.
///
/// # Prerequisites
/// The type to wrap needs to implement `Default`, to be movable in C++.
///
/// # Example
/// ```rust
/// struct Foo {
///    a: u32,
/// }
/// make_option_type!(Foo); // Generates a `OptionFoo` that can be bridged to C.
/// // Alternatively define your own name for the Option type.
/// make_option_type!(u32, OptionU32);
/// // Alternatively you can also pass in additional traits the OptionT should derive.
/// make_option_type!(u64, OptionU64, PartialEq);
/// ...
/// let option_foo = foo.into();
/// ...
/// ```
#[macro_export]
macro_rules! make_option_type {
    ($type:ty, $option_name:ident $( < $lt:lifetime > )? ) => {
        #[derive(Debug)]
        pub struct $option_name $( <$lt> )? {
            pub(crate) inner: Option<$type>,
        }

        impl $( <$lt> )? $option_name $( <$lt> )? {
            pub fn from_some(some: $type) -> Self {
                Self { inner: Some(some) }
            }

            pub fn is_some(&self) -> bool {
                self.inner.is_some()
            }

            pub fn is_none(&self) -> bool {
                self.inner.is_none()
            }

            pub fn unwrap(self) -> $type {
                self.inner.unwrap()
            }

            pub fn unwrap_as_ref(&self) -> &$type {
                self.inner.as_ref().unwrap()
            }

            pub fn unwrap_or(self, default: $type) -> $type {
                self.inner.unwrap_or(default)
            }
        }

        impl $( <$lt> )? From<Option<$type>> for $option_name $( <$lt> )? {
            fn from(val: Option<$type>) -> Self {
                Self { inner: val }
            }
        }

        impl $( <$lt> )? From<$type> for $option_name $( <$lt> )? {
            fn from(val: $type) -> Self {
                Self { inner: Some(val) }
            }
        }

        #[doc = "This default implementation only exists to make the resulting Rust type movable."]
        #[doc = "For more information, see go/crubit/rust/movable_types."]
        impl $( <$lt> )? std::default::Default for $option_name $( <$lt> )? {
            fn default() -> Self {
                Self { inner: None }
            }
        }
    };
    ($type:ident  $( < $lt:lifetime > )?) => {
        paste::paste! {
          make_option_type!($type $( <$lt> )?, [<Option$type>] $( <$lt> )?);
        }
    };
}

// Macro to allow for error propagation
// go/crubit/overview/unstable_features#try-trait-v2
#[macro_export]
macro_rules! try_crubit {
    ($block:block) => {
        (|| -> anyhow::Result<_> { $block })().into()
    };
}

// Macro to facilitate creating wrapper types.
#[macro_export]
macro_rules! make_item_wrapper {
    ($(#[$struct_doc:meta])* $inner_name:ty, $wrapper_name:ident $(<$lt:lifetime>)?, { $( $(#[$method_doc:meta])* $field:ident -> $ret_type:ty),* $(,)? }) => {
        paste::paste! {
            $(#[$struct_doc])*
            #[derive(Debug, Clone)]
            pub struct $wrapper_name $(<$lt>)? {
                inner: $inner_name,

            }

            impl $(<$lt>)? From<$inner_name> for $wrapper_name $(<$lt>)? {
                fn from(inner: $inner_name) -> Self {
                    Self { inner }
                }
            }

            impl $(<$lt>)? $wrapper_name $(<$lt>)? {
                $(
                    $(#[$method_doc:meta])*
                    pub fn $field(&self) -> $ret_type {
                        self.inner.$field
                    }
                )*
            }

            make_option_type!($wrapper_name $(<$lt>)?);
            make_result_type!($wrapper_name $(<$lt>)?);
            make_result_type!([<Option $wrapper_name>] $(<$lt>)?);
        }
    }
}

// Macro to facilitate creating wrapper types of the generic ParsingTable
#[macro_export]
macro_rules! make_table_wrapper {
    (
        $(#[$struct_doc:meta])*
        $inner_table_type:ty,
        $wrapper_name:ident $(<$lt:lifetime>)?,
        $wrapper_item_name:ident,
        $result_item_type:ident
    ) => {
        paste::paste! {
            $(#[$struct_doc])*
            #[derive(Debug, Clone)]
            pub struct $wrapper_name $(<$lt>)? {
                inner: $inner_table_type,
            }

            impl $(<$lt>)? $wrapper_name $(<$lt>)? {
                pub fn len(&self) -> usize {
                    self.inner.len()
                }

                pub fn is_empty(&self) -> bool {
                    self.len() == 0
                }

                pub fn get(&self, index: usize) -> $result_item_type  {
                    try_crubit!({
                        let item = self.inner.get(index)?;
                        Ok($wrapper_item_name { inner: item,  })
                    })
                }
            }

            make_option_type!($wrapper_name $(<$lt>)?);
            make_result_type!($wrapper_name $(<$lt>)?);
            make_result_type!([<Option $wrapper_name>] $(<$lt>)?);
        }
    };
}

// Macro to facilitate creating wrapper types for ParsingIterator
#[macro_export]
macro_rules! make_iterator_wrapper {
    (
        $(#[$struct_doc:meta])*
        $inner_iterator_type:ty,
        $wrapper_name:ident $(<$lt:lifetime>)?,
        $wrapper_item_name:ident,
    ) => {
        paste::paste! {
            $(#[$struct_doc])*
            #[derive(Debug)]
            pub struct $wrapper_name $(<$lt>)? {
                inner: $inner_iterator_type,
            }

            impl$(<$lt>)? $wrapper_name$(<$lt>)? {
                /// Advances the iterator and returns the next value.
                pub fn next(&mut self) -> [<Option $wrapper_item_name>] {
                    self.inner.next().map(|item| $wrapper_item_name {
                        inner: item,

                    }).into()
                }
            }

            make_option_type!($wrapper_name$(<$lt>)?);
            make_result_type!($wrapper_name$(<$lt>)?);
            make_result_type!([<Option $wrapper_name>]$(<$lt>)?);
        }
    };
}
