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
            inner: anyhow::Result<$type>,
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
    ($type:ty, $option_name:ident $(, $($derive_trait:ident),*)?) => {
        #[derive(Debug, Default $(, $($derive_trait),* )? )]
        pub struct $option_name {
            inner: Option<$type>,
        }

        impl $option_name {
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

            pub fn unwrap_or(self, default: $type) -> $type {
                self.inner.unwrap_or(default)
            }
        }

        impl From<Option<$type>> for $option_name {
            fn from(val: Option<$type>) -> Self {
                Self { inner: val }
            }
        }

        impl From<$type> for $option_name {
            fn from(val: $type) -> Self {
                Self { inner: Some(val) }
            }
        }
    };
    ($option_name:ident) => {
        paste::paste! {
          make_option_type!($option_name, [<Option$option_name>]);
        }
    };
}

#[cfg(test)]
mod test {
        use anyhow::anyhow;
    use googletest::prelude::*;

    #[derive(Debug, PartialEq, Clone, Default)]
    pub struct Demo {
        val: u32,
    }
    make_result_type!(Demo);

    #[derive(Debug, PartialEq, Clone, Copy)]
    pub struct HoldsRef<'a> {
        val: &'a str,
    }
    make_result_type!(HoldsRef<'a>, ResultHoldsRef<'a>);

    #[derive(Debug, PartialEq, Clone, Copy, Default, PartialOrd)]
    pub struct CopyDemo {
        val: u32,
    }
    make_option_type!(CopyDemo);

    #[gtest]
    fn test_inherent_methods_result() {
        // Ok() case.
        let val: Demo = Demo { val: 42 };
        let result = ResultDemo { inner: Ok(val.clone()) };
        expect_true!(result.is_ok());
        expect_false!(result.is_err());
        expect_eq!(result.unwrap(), val.clone());

        let result = ResultDemo::from_ok(val.clone());
        expect_true!(result.is_ok());
        expect_false!(result.is_err());
        expect_eq!(result.unwrap(), val.clone());

        // Err() case.
        let result = ResultDemo { inner: Err(anyhow!("error")) };
        expect_false!(result.is_ok());
        expect_true!(result.is_err());
        expect_eq!(result.unwrap_err(), cc_std::std::string::from("error"));
        let result = ResultDemo { inner: Err(anyhow!("error")) };
        expect_eq!(result.unwrap_or(Demo { val: 43 }), Demo { val: 43 });
    }

    #[gtest]
    fn test_inherent_methods_result_lifetime() -> Result<()> {
        let test_string = String::from("abcd");
        // Ok() case.
        let val = HoldsRef { val: test_string.as_str() };
        let result = ResultHoldsRef { inner: Ok(val) };
        verify_true!(result.is_ok())?;
        expect_false!(result.is_err());
        expect_eq!(result.unwrap(), val);

        let result = ResultHoldsRef::from_ok(val);
        verify_true!(result.is_ok())?;
        expect_false!(result.is_err());
        expect_eq!(result.unwrap(), val);

        // Err() case.
        let result = ResultHoldsRef { inner: Err(anyhow!("error")) };
        expect_false!(result.is_ok());
        verify_true!(result.is_err())?;
        expect_eq!(result.unwrap_err(), cc_std::std::string::from("error"));
        let result = ResultHoldsRef { inner: Err(anyhow!("error")) };
        expect_eq!(
            result.unwrap_or(HoldsRef { val: test_string.as_str() }),
            HoldsRef { val: test_string.as_str() }
        );

        Ok(())
    }

    #[gtest]
    fn test_conversions_result() {
        let val: Demo = Demo { val: 42 };
        let result = ResultDemo::from(Ok(val.clone()));
        expect_true!(result.is_ok());
        expect_eq!(result.unwrap(), val.clone());

        let result = ResultDemo::from(val.clone());
        expect_true!(result.is_ok());
        expect_eq!(result.unwrap(), val);

        let result = ResultDemo::from(anyhow::Error::msg("error"));
        expect_false!(result.is_ok());
        expect_true!(result.is_err());
        expect_eq!(result.unwrap_err(), cc_std::std::string::from("error"));
    }

    #[gtest]
    fn test_conversions_result_lifetime() -> Result<()> {
        let val = HoldsRef { val: "test" };
        let result = ResultHoldsRef::from(Ok(val));
        verify_true!(result.is_ok())?;
        expect_eq!(result.unwrap(), val);

        let result = ResultHoldsRef::from(val);
        verify_true!(result.is_ok())?;
        expect_eq!(result.unwrap(), val);

        let result = ResultHoldsRef::from(anyhow::Error::msg("error"));
        expect_false!(result.is_ok());
        verify_true!(result.is_err())?;
        expect_eq!(result.unwrap_err(), cc_std::std::string::from("error"));

        Ok(())
    }

    #[gtest]
    fn test_inherent_methods_option() {
        // Some() case.
        let val: CopyDemo = CopyDemo { val: 42 };
        let option = OptionCopyDemo { inner: Some(val.clone()) };
        expect_true!(option.is_some());
        expect_false!(option.is_none());
        expect_eq!(option.unwrap(), val.clone());

        let option = OptionCopyDemo::from_some(val.clone());
        expect_true!(option.is_some());
        expect_false!(option.is_none());
        expect_eq!(option.unwrap(), val);

        // None case.
        let option = OptionCopyDemo { inner: None };
        expect_false!(option.is_some());
        expect_true!(option.is_none());
        expect_eq!(option.unwrap_or(CopyDemo { val: 43 }), CopyDemo { val: 43 });
    }

    #[gtest]
    fn test_conversions_option() {
        let val: CopyDemo = CopyDemo { val: 42 };
        let option = OptionCopyDemo::from(val.clone());
        expect_true!(option.is_some());
        expect_eq!(option.unwrap(), val.clone());

        let option = OptionCopyDemo::from(Some(val.clone()));
        expect_true!(option.is_some());
        expect_eq!(option.unwrap(), val);

        let option = OptionCopyDemo::from(None);
        expect_false!(option.is_some());
        expect_true!(option.is_none());
    }
}
