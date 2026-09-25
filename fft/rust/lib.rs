//! This crate provides C++ bindings for the ndrustfft crate.
//!
//! WARNING: This crate should never be used from Rust. Use ndrustfft directly.
//!
//! The ndrustfft API is structured as a set of functions that are generic in two ways:
//!
//! - They take ndarray::ArrayBase objects as inputs/outputs, which can be of any dimensionality.
//! - They can work on both f32 and f64 arrays (or Complex<f32> and Complex<f64> where applicable).
//!
//! In order to have functions that Crubit can export to C++, we expand each generic function into
//! two variants, with "_f32" and "_f64" suffixes. We also pass the input and output arrays as
//! flat slices of wrapped (because Crubit doesn't support Complex<T>) number types, with the shape
//! as an additional arguments. So for example, instead of
//! ```
//!     pub fn ndfft<R, S, T, D>(
//!         input: &ArrayBase<R, D>,
//!         output: &mut ArrayBase<S, D>,
//!         handler: &FftHandler<T>,
//!         axis: usize,
//!     )
//! ```
//! We export the following two variants:
//! ```
//!     fft_f64(shape: &[usize],
//!         input: &[ComplexF64], inembed: &[usize], istride: usize,
//!         output: &mut [ComplexF64], onembed: &[usize], ostride: usize,
//!         handler: &FftHandlerF64, axis: usize)
//!     fft_f32(shape: &[usize],
//!         input: &[ComplexF32], inembed: &[usize], istride: usize,
//!         output: &mut [ComplexF32], onembed: &[usize], ostride: usize,
//!         handler: FftHandlerF32, axis: usize)
//! ```
//! This is also similar to how C++ code using FFTW passes buffers, using function arguments to
//! pass a pointer and the size of each axis.
//!
//! The input/output `nembed` and `stride` arguments allow specifying non-contiguous arrays, with
//! semantics similar to the FFTW function arguments with the same names:
//!
//! - `nembed` is the shape of a larger array that contains the subarray we're transforming. It
//!   must have the same dimensions as `shape` and each dimension must be of greater or equal
//!   value than the corresponding dimension in `shape`.Passing the same value as `shape` means
//!   we're transforming the whole array.
//!
//! - `stride` is the index increment to go from one element in a row to the next. For example, if
//!   you have an image where color is represented by interleaved [R, G, B, R, G, B...] values, you
//!   can transform only the red channel by using `stride = 3`. A value of 1 means the input values
//!   are consecutive.
//!
//! The wrapper internally constructs `ArrayView` objects of the specified dimensions, which can
//! fail if the dimensions are incompatible with the slice size. So these wrapper functions return
//! a Result<(), Vec<u8>> instead of nothing, to account for the possibility of failure.

use bytemuck::TransparentWrapper;
use ndarray::{ArrayViewD, ArrayViewMutD, IxDyn, ShapeBuilder};
use ndrustfft::{
    nddct1, nddct1_inplace, nddct2, nddct2_inplace, nddct3, nddct3_inplace, nddct4, nddct4_inplace,
    ndfft, ndfft_inplace, ndfft_r2c, ndifft, ndifft_inplace, ndifft_r2c, Complex, DctHandler,
    FftHandler, Normalization, R2cFftHandler,
};

pub use ndarray::Slice;

// Monomorphized Complex types for crubit.
#[repr(transparent)]
pub struct ComplexF64(pub Complex<f64>);

// SAFETY: Trivially matches safety contract of TransparentWrapper.
unsafe impl TransparentWrapper<Complex<f64>> for ComplexF64 {}

#[repr(transparent)]
pub struct ComplexF32(pub Complex<f32>);

// SAFETY: Trivially matches safety contract of TransparentWrapper.
unsafe impl TransparentWrapper<Complex<f32>> for ComplexF32 {}

// Wrappers for real numbers so we can treat them the same as complex numbers in macros.
#[repr(transparent)]
pub struct RealF64(pub f64);

// SAFETY: Trivially matches safety contract of TransparentWrapper.
unsafe impl TransparentWrapper<f64> for RealF64 {}

#[repr(transparent)]
pub struct RealF32(pub f32);

// SAFETY: Trivially matches safety contract of TransparentWrapper.
unsafe impl TransparentWrapper<f32> for RealF32 {}

// Monomorphized *Handler types for Crubit.
#[derive(Clone)]
pub struct DctHandlerF64(pub(crate) DctHandler<f64>);

impl DctHandlerF64 {
    pub fn new(n: usize) -> Self {
        DctHandlerF64(DctHandler::<f64>::new(n))
    }

    pub fn normalization(self, use_default_normalization: bool) -> Self {
        let norm =
            if use_default_normalization { Normalization::Default } else { Normalization::None };
        DctHandlerF64(self.0.normalization(norm))
    }
}

#[derive(Clone)]
pub struct DctHandlerF32(pub(crate) DctHandler<f32>);

impl DctHandlerF32 {
    pub fn new(n: usize) -> Self {
        DctHandlerF32(DctHandler::<f32>::new(n))
    }

    pub fn normalization(self, use_default_normalization: bool) -> Self {
        let norm =
            if use_default_normalization { Normalization::Default } else { Normalization::None };
        DctHandlerF32(self.0.normalization(norm))
    }
}

#[derive(Clone)]
pub struct FftHandlerF64(pub(crate) FftHandler<f64>);

impl FftHandlerF64 {
    pub fn new(n: usize) -> Self {
        FftHandlerF64(FftHandler::<f64>::new(n))
    }

    pub fn normalization(self, use_default_normalization: bool) -> Self {
        let norm =
            if use_default_normalization { Normalization::Default } else { Normalization::None };
        FftHandlerF64(self.0.normalization(norm))
    }
}

#[derive(Clone)]
pub struct FftHandlerF32(pub(crate) FftHandler<f32>);

impl FftHandlerF32 {
    pub fn new(n: usize) -> Self {
        FftHandlerF32(FftHandler::<f32>::new(n))
    }

    pub fn normalization(self, use_default_normalization: bool) -> Self {
        let norm =
            if use_default_normalization { Normalization::Default } else { Normalization::None };
        FftHandlerF32(self.0.normalization(norm))
    }
}

#[derive(Clone)]
pub struct R2cFftHandlerF64(pub(crate) R2cFftHandler<f64>);

impl R2cFftHandlerF64 {
    pub fn new(n: usize) -> Self {
        R2cFftHandlerF64(R2cFftHandler::<f64>::new(n))
    }

    pub fn normalization(self, use_default_normalization: bool) -> Self {
        let norm =
            if use_default_normalization { Normalization::Default } else { Normalization::None };
        R2cFftHandlerF64(self.0.normalization(norm))
    }
}

#[derive(Clone)]
pub struct R2cFftHandlerF32(pub(crate) R2cFftHandler<f32>);

impl R2cFftHandlerF32 {
    pub fn new(n: usize) -> Self {
        R2cFftHandlerF32(R2cFftHandler::<f32>::new(n))
    }

    pub fn normalization(self, use_default_normalization: bool) -> Self {
        let norm =
            if use_default_normalization { Normalization::Default } else { Normalization::None };
        R2cFftHandlerF32(self.0.normalization(norm))
    }
}

macro_rules! define_fftw_view {
    ($func_name:ident, $view_type:ident $(, $mutability:tt)?) => {
        // Creates a $view_type `data` using the shape given by the arguments.
        //
        // `shape`: the shape of the resulting subarray.
        // `nembed`: the shape of the larger array that we're taking a subarray of.
        // `stride`: index increment between consecutive elements in a row.
        fn $func_name<'a, T>(
            data: &'a $($mutability)? [T],
            shape: &[usize],
            nembed: &[usize],
            stride: usize,
        ) -> Result<$view_type<'a, T>, Vec<u8>> {
            if shape.len() != nembed.len() {
                return Err(
                    b"shape and nembed must have the same number of dimensions (rank)".to_vec(),
                );
            }

            let ndim = shape.len();
            let mut strides = vec![0; ndim];
            let mut current_stride = stride;

            for i in (0..ndim).rev() {
                strides[i] = current_stride;
                if i > 0 {
                    current_stride *= nembed[i];
                }
            }

            let dim = IxDyn(shape);
            let custom_strides = IxDyn(&strides);
            $view_type::from_shape(dim.strides(custom_strides), data)
                .map_err(|e| e.to_string().into_bytes())
        }
    }
}

define_fftw_view!(create_fftw_view, ArrayViewD);
define_fftw_view!(create_fftw_view_mut, ArrayViewMutD, mut);

macro_rules! expand_transform_variants {
    ($($(#[doc = $doc:expr])*
    $func:ident(shape: &[usize],
                input: &[$in_type:ident],
                inembed: &[usize],
                istride: usize,
                output: &mut [$out_type:ident],
                onembed: &[usize],
                ostride: usize,
                handler: $handler_type:ident,
                axis: usize) => $ndfunc:ident,)*) => {
        $($(#[doc = $doc])*
        #[allow(clippy::too_many_arguments)]
        pub fn $func(
            shape: &[usize],
            input: &[$in_type],
            inembed: &[usize],
            istride: usize,
            output: &mut [$out_type],
            onembed: &[usize],
            ostride: usize,
            handler: &$handler_type,
            axis: usize,
        ) -> Result<(), Vec<u8>> {
            if axis >= shape.len() {
                return Err(format!(
                    "axis ({}) must be less than the number of dimensions ({})",
                    axis,
                    shape.len()
                )
                .into_bytes());
            }
            let input_array = create_fftw_view(
                $in_type::peel_slice(input), shape, inembed, istride,
            )?;
            let mut output_array = create_fftw_view_mut(
                $out_type::peel_slice_mut(output), shape, onembed, ostride,
            )?;
            $ndfunc(&input_array, &mut output_array, &handler.0, axis);
            Ok(())
        })*
    };
}

macro_rules! expand_transform_inplace_variants {
    ($($(#[doc = $doc:expr])*
    $func:ident(shape: &[usize],
                data: &mut [$type:ident],
                nembed: &[usize],
                stride: usize,
                handler: $handler_type:ident,
                axis: usize) => $ndfunc:ident,)*) => {
        $($(#[doc = $doc])*
        pub fn $func(
            shape: &[usize],
            data: &mut [$type],
            nembed: &[usize],
            stride: usize,
            handler: &$handler_type,
            axis: usize,
        ) -> Result<(), Vec<u8>> {
            if axis >= shape.len() {
                return Err(format!(
                    "axis ({}) must be less than the number of dimensions ({})",
                    axis,
                    shape.len()
                )
                .into_bytes());
            }
            let mut data_array = create_fftw_view_mut(
                $type::peel_slice_mut(data), shape, nembed, stride,
            )?;
            $ndfunc(&mut data_array, &handler.0, axis);
            Ok(())
        })*
    };
}

// Real-to-complex FFT functions are not n-to-n. They exploit the fact that the output is symmetric
// and only return n/2 + 1 values.
macro_rules! expand_r2c_transform_variants {
    ($($(#[doc = $doc:expr])*
    $func:ident(shape: &[usize],
                input: &[$in_type:ident],
                inembed: &[usize],
                istride: usize,
                output: &mut [$out_type:ident],
                onembed: &[usize],
                ostride: usize,
                handler: $handler_type:ident,
                axis: usize) => $ndfunc:ident,)*) => {
        $($(#[doc = $doc])*
        #[allow(clippy::too_many_arguments)]
        pub fn $func(
            shape: &[usize],
            input: &[$in_type],
            inembed: &[usize],
            istride: usize,
            output: &mut [$out_type],
            onembed: &[usize],
            ostride: usize,
            handler: &$handler_type,
            axis: usize,
        ) -> Result<(), Vec<u8>> {
            if axis >= shape.len() {
                return Err(format!(
                    "axis ({}) must be less than the number of dimensions ({})",
                    axis,
                    shape.len()
                )
                .into_bytes());
            }
            let input_array = create_fftw_view(
                $in_type::peel_slice(input), shape, inembed, istride)?;
            // The output of a real-to-complex transform contains symmetries that make half of
            // the outputs redundant. Therefore, ndrustfft (like FFTW) will return n/2+1 complex
            // numbers for a r2c transform of n reals. See the explanation in
            // https://www.fftw.org/fftw3_doc/Real_002ddata-DFT-Array-Format.html
            //
            // In the multi-dimensional case, FFTW always applies this "halving" to the last
            // dimension. However, ndrustfft only applies the transform along one axis, so
            // this change in size happens along the chosen axis.
            let mut output_shape = shape.to_vec();
            output_shape[axis] = output_shape[axis] / 2 + 1;
            let mut output_array = create_fftw_view_mut(
                $out_type::peel_slice_mut(output),
                output_shape.as_slice(), onembed, ostride,
            )?;
            $ndfunc(&input_array, &mut output_array, &handler.0, axis);
            Ok(())
        })*
    };
}

// Complex-to-real functions are also not n-to-n. They take as input the non-redundant half of the
// complex array (like the output of a real-to-complex transform), and return double the number of
// real values. The `n` argument is still the original size, so we need to apply the (n/2+1)
// reduction to the input dimensions.
macro_rules! expand_c2r_transform_variants {
    ($($(#[doc = $doc:expr])*
    $func:ident(shape: &[usize],
                input: &[$in_type:ident],
                inembed: &[usize],
                istride: usize,
                output: &mut [$out_type:ident],
                onembed: &[usize],
                ostride: usize,
                handler: $handler_type:ident,
                axis: usize) => $ndfunc:ident,)*) => {
        $($(#[doc = $doc])*
        #[allow(clippy::too_many_arguments)]
        pub fn $func(
            shape: &[usize],
            input: &[$in_type],
            inembed: &[usize],
            istride: usize,
            output: &mut [$out_type],
            onembed: &[usize],
            ostride: usize,
            handler: &$handler_type,
            axis: usize,
        ) -> Result<(), Vec<u8>> {
            if axis >= shape.len() {
                return Err(format!(
                    "axis ({}) must be less than the number of dimensions ({})",
                    axis,
                    shape.len()
                )
                .into_bytes());
            }

            let mut input_shape = shape.to_vec();
            input_shape[axis] = input_shape[axis] / 2 + 1;
            let input_array = create_fftw_view(
                $in_type::peel_slice(input), input_shape.as_slice(), inembed, istride,
            )?;
            let mut output_array = create_fftw_view_mut(
                $out_type::peel_slice_mut(output), shape, onembed, ostride,
            )?;
            $ndfunc(&input_array, &mut output_array, &handler.0, axis);
            Ok(())
        })*
    };
}

// Real-to-real transforms (DCT).
expand_transform_variants! {
    /// Computes a DCT-I along the given axis.
    dct1_f64(shape: &[usize],
             input: &[RealF64], inembed: &[usize], istride: usize,
             output: &mut [RealF64], onembed: &[usize], ostride: usize,
             handler: DctHandlerF64, axis: usize) => nddct1,
    /// Computes a DCT-I along the given axis.
    dct1_f32(shape: &[usize],
             input: &[RealF32], inembed: &[usize], istride: usize,
             output: &mut [RealF32], onembed: &[usize], ostride: usize,
             handler: DctHandlerF32, axis: usize) => nddct1,

    /// Computes a DCT-II ("the" DCT) along the given axis.
    dct2_f64(shape: &[usize],
             input: &[RealF64], inembed: &[usize], istride: usize,
             output: &mut [RealF64], onembed: &[usize], ostride: usize,
             handler: DctHandlerF64, axis: usize) => nddct2,
    /// Computes a DCT-II ("the" DCT) along the given axis.
    dct2_f32(shape: &[usize],
             input: &[RealF32], inembed: &[usize], istride: usize,
             output: &mut [RealF32], onembed: &[usize], ostride: usize,
             handler: DctHandlerF32, axis: usize) => nddct2,

    /// Computes a DCT-III ("the" IDCT) along the given axis.
    dct3_f64(shape: &[usize],
             input: &[RealF64], inembed: &[usize], istride: usize,
             output: &mut [RealF64], onembed: &[usize], ostride: usize,
             handler: DctHandlerF64, axis: usize) => nddct3,
    /// Computes a DCT-III ("the" IDCT) along the given axis.
    dct3_f32(shape: &[usize],
             input: &[RealF32], inembed: &[usize], istride: usize,
             output: &mut [RealF32], onembed: &[usize], ostride: usize,
             handler: DctHandlerF32, axis: usize) => nddct3,

    /// Computes a DCT-IV along the given axis.
    dct4_f64(shape: &[usize],
             input: &[RealF64], inembed: &[usize], istride: usize,
             output: &mut [RealF64], onembed: &[usize], ostride: usize,
             handler: DctHandlerF64, axis: usize) => nddct4,
    /// Computes a DCT-IV along the given axis.
    dct4_f32(shape: &[usize],
             input: &[RealF32], inembed: &[usize], istride: usize,
             output: &mut [RealF32], onembed: &[usize], ostride: usize,
             handler: DctHandlerF32, axis: usize) => nddct4,
}

// In-place real-to-real transforms (DCT).
expand_transform_inplace_variants! {
    /// Computes a DCT-I in place along the given axis.
    dct1_f64_inplace(shape: &[usize],
                     data: &mut [RealF64], nembed: &[usize], stride: usize,
                     handler: DctHandlerF64, axis: usize) => nddct1_inplace,
    /// Computes a DCT-I in place along the given axis.
    dct1_f32_inplace(shape: &[usize],
                     data: &mut [RealF32], nembed: &[usize], stride: usize,
                     handler: DctHandlerF32, axis: usize) => nddct1_inplace,
    /// Computes a DCT-II ("the" DCT) in place along the given axis.
    dct2_f64_inplace(shape: &[usize],
                     data: &mut [RealF64], nembed: &[usize], stride: usize,
                     handler: DctHandlerF64, axis: usize) => nddct2_inplace,
    /// Computes a DCT-II ("the" DCT) in place along the given axis.
    dct2_f32_inplace(shape: &[usize],
                     data: &mut [RealF32], nembed: &[usize], stride: usize,
                     handler: DctHandlerF32, axis: usize) => nddct2_inplace,
    /// Computes a DCT-III ("the" IDCT) in place along the given axis.
    dct3_f64_inplace(shape: &[usize],
                     data: &mut [RealF64], nembed: &[usize], stride: usize,
                     handler: DctHandlerF64, axis: usize) => nddct3_inplace,
    /// Computes a DCT-III ("the" IDCT) in place along the given axis.
    dct3_f32_inplace(shape: &[usize],
                     data: &mut [RealF32], nembed: &[usize], stride: usize,
                     handler: DctHandlerF32, axis: usize) => nddct3_inplace,
    /// Computes a DCT-IV in place along the given axis.
    dct4_f64_inplace(shape: &[usize],
                     data: &mut [RealF64], nembed: &[usize], stride: usize,
                     handler: DctHandlerF64, axis: usize) => nddct4_inplace,
    /// Computes a DCT-IV in place along the given axis.
    dct4_f32_inplace(shape: &[usize],
                     data: &mut [RealF32], nembed: &[usize], stride: usize,
                     handler: DctHandlerF32, axis: usize) => nddct4_inplace,
}

// Complex-to-complex transforms (FFT and IFFT).
expand_transform_variants! {
    /// Computes an FFT along the given axis.
    fft_f64(shape: &[usize],
            input: &[ComplexF64], inembed: &[usize], istride: usize,
            output: &mut [ComplexF64], onembed: &[usize], ostride: usize,
            handler: FftHandlerF64, axis: usize) => ndfft,
    /// Computes an FFT along the given axis.
    fft_f32(shape: &[usize],
            input: &[ComplexF32], inembed: &[usize], istride: usize,
            output: &mut [ComplexF32], onembed: &[usize], ostride: usize,
            handler: FftHandlerF32, axis: usize) => ndfft,

    /// Computes an inverse FFT along the given axis.
    ifft_f64(shape: &[usize],
             input: &[ComplexF64], inembed: &[usize], istride: usize,
             output: &mut [ComplexF64], onembed: &[usize], ostride: usize,
             handler: FftHandlerF64, axis: usize) => ndifft,
    /// Computes an inverse FFT along the given axis.
    ifft_f32(shape: &[usize],
             input: &[ComplexF32], inembed: &[usize], istride: usize,
             output: &mut [ComplexF32], onembed: &[usize], ostride: usize,
             handler: FftHandlerF32, axis: usize) => ndifft,
}

// In-place complex-to-complex transforms (FFT and IFFT).
expand_transform_inplace_variants! {
    /// Computes an FFT in place along the given axis.
    fft_f64_inplace(shape: &[usize],
                     data: &mut [ComplexF64], nembed: &[usize], stride: usize,
                     handler: FftHandlerF64, axis: usize) => ndfft_inplace,
    /// Computes an FFT in place along the given axis.
    fft_f32_inplace(shape: &[usize],
                     data: &mut [ComplexF32], nembed: &[usize], stride: usize,
                     handler: FftHandlerF32, axis: usize) => ndfft_inplace,
    /// Computes an inverse FFT in place along the given axis.
    ifft_f64_inplace(shape: &[usize],
                      data: &mut [ComplexF64], nembed: &[usize], stride: usize,
                      handler: FftHandlerF64, axis: usize) => ndifft_inplace,
    /// Computes an inverse FFT in place along the given axis.
    ifft_f32_inplace(shape: &[usize],
                      data: &mut [ComplexF32], nembed: &[usize], stride: usize,
                      handler: FftHandlerF32, axis: usize) => ndifft_inplace,
}

// Real-to-complex FFT variants.
expand_r2c_transform_variants! {
    /// Computes a real-to-complex FFT along the given axis.
    fft_r2c_f64(shape: &[usize],
                input: &[RealF64], inembed: &[usize], istride: usize,
                output: &mut [ComplexF64], onembed: &[usize], ostride: usize,
                handler: R2cFftHandlerF64, axis: usize) => ndfft_r2c,
    /// Computes a real-to-complex FFT along the given axis.
    fft_r2c_f32(shape: &[usize],
                input: &[RealF32], inembed: &[usize], istride: usize,
                output: &mut [ComplexF32], onembed: &[usize], ostride: usize,
                handler: R2cFftHandlerF32, axis: usize) => ndfft_r2c,
}

// Note these are INVERSE r2c transforms. This means they're actually complex-to-real
// despite the "r2c" in the name. This follows the ndrustfft naming scheme.
expand_c2r_transform_variants! {
    ifft_r2c_f64(shape: &[usize],
                 input: &[ComplexF64], inembed: &[usize], istride: usize,
                 output: &mut [RealF64], onembed: &[usize], ostride: usize,
                 handler: R2cFftHandlerF64, axis: usize) => ndifft_r2c,
    ifft_r2c_f32(shape: &[usize],
                 input: &[ComplexF32], inembed: &[usize], istride: usize,
                 output: &mut [RealF32], onembed: &[usize], ostride: usize,
                 handler: R2cFftHandlerF32, axis: usize) => ndifft_r2c,
}

#[cfg(test)]
mod tests {
        use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn test_create_fftw_view_standard() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let view = create_fftw_view(&data, &[2, 3], &[2, 3], 1).unwrap();
        assert_eq!(view.shape(), &[2, 3]);
        assert_eq!(view.strides(), &[3, 1]);

        assert_eq!(view[[0, 0]], 1.0);
        assert_eq!(view[[0, 1]], 2.0);
        assert_eq!(view[[0, 2]], 3.0);
        assert_eq!(view[[1, 0]], 4.0);
        assert_eq!(view[[1, 1]], 5.0);
        assert_eq!(view[[1, 2]], 6.0);
    }

    #[gtest]
    fn test_create_fftw_view_mut() {
        let mut data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mut view = create_fftw_view_mut(&mut data, &[2, 3], &[2, 3], 1).unwrap();
        assert_eq!(view.shape(), &[2, 3]);
        assert_eq!(view.strides(), &[3, 1]);

        view[[0, 0]] = 10.0;
        assert_eq!(data[0], 10.0);
    }

    #[gtest]
    fn test_create_fftw_view_subarray() {
        let data = vec![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        ];

        let view = create_fftw_view(&data, &[2, 2], &[4, 4], 1).unwrap();
        assert_eq!(view.shape(), &[2, 2]);
        assert_eq!(view.strides(), &[4, 1]);

        assert_eq!(view[[0, 0]], 1.0);
        assert_eq!(view[[0, 1]], 2.0);
        assert_eq!(view[[1, 0]], 5.0);
        assert_eq!(view[[1, 1]], 6.0);
    }

    #[gtest]
    fn test_create_fftw_view_with_stride() {
        let data = vec![1.0, 0.0, 2.0, 0.0, 3.0, 0.0, 4.0, 0.0];

        let view = create_fftw_view(&data, &[2, 2], &[2, 2], 2).unwrap();
        assert_eq!(view.shape(), &[2, 2]);
        assert_eq!(view.strides(), &[4, 2]);

        assert_eq!(view[[0, 0]], 1.0);
        assert_eq!(view[[0, 1]], 2.0);
        assert_eq!(view[[1, 0]], 3.0);
        assert_eq!(view[[1, 1]], 4.0);
    }

    #[gtest]
    fn test_create_fftw_view_subarray_and_stride() {
        #[rustfmt::skip]
        let data = vec![
             1.0,  2.0,  3.0,  4.0,  5.0,  6.0,  7.0,  8.0,
             9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
            17.0, 18.0, 19.0, 20.0, 21.0, 22.0, 23.0, 24.0,
            25.0, 26.0, 27.0, 28.0, 29.0, 30.0, 31.0, 32.0,
        ];

        // The original matrix is 4x8, but we skip every other element (stride=2)
        // along the last dimension. So we have a 4x4 matrix, and we're taking a
        // 2x2 subarray out of it.
        let view = create_fftw_view(&data, &[2, 2], &[4, 4], 2).unwrap();
        assert_eq!(view.shape(), &[2, 2]);
        assert_eq!(view.strides(), &[8, 2]);

        assert_eq!(view[[0, 0]], 1.0);
        assert_eq!(view[[0, 1]], 3.0);
        assert_eq!(view[[1, 0]], 9.0);
        assert_eq!(view[[1, 1]], 11.0);
    }
}
