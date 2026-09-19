// C++ wrapper around the ndrustfft library.

// This library provides a C++ interface to the ndrustfft library, which
// contains Rust implementations of the Fast Fourier Transform (FFT),
// real-to-complex FFT, Discrete Cosine Transform (DCT), and their inverses.
// Each transform is supported in N dimensions, for both float and double
// precision.
//
// Each transform is a stand-alone function, which takes a plan as an argument.
// Each kind of transform has its own plan class (FftPlan, DctPlan...), and the
// same plan class works for a transform and its inverse. The plan is
// specialized for a given transform size and can be reused for multiple
// executions of the same size and kind.
//
// 1D example:
//
//   std::vector<std::complex<double>> input = {...};
//   std::vector<std::complex<double>> output(input.size(), {0, 0});
//   FftPlan<double, 1> plan({input.size()});
//   ASSERT_OK(Fft(absl::MakeConstSpan(input), absl::MakeSpan(output), plan));
//
// 2D example:
//
//   std::vector<std::complex<double>> input = {...};
//   std::vector<std::complex<double>> output(6, {0, 0});
//   FftPlan<double, 2> plan({2, 3});
//   ASSERT_OK(Fft(absl::MakeConstSpan(input), absl::MakeSpan(output), plan));
//
// Transform functions return a status, which can be an error if the input
// dimensions do not match the plan.
//
// Note that the input and output matrices are passed as flat Spans. The actual
// dimensions of the transform are passed to the plan constructor.
//
// Each transform function has an "advanced" overload that takes ArrayLayout
// objects to define the input and output arrays. This allows processing
// arrays with non-contiguous elements or subarrays of larger arrays. See
// `security/fft/array_layout.h` for more information.
//
// See fft_test.cc for more examples.

#ifndef SECURITY_FFT_FFT_H_
#define SECURITY_FFT_FFT_H_

#include <array>
#include <complex>
#include <cstddef>
#include <type_traits>
#include <utility>
#include <vector>

#include "array_layout.h"
#include "fft_internal.h"
#include "crubit/rust.h"
#include "absl/status/status.h"
#include "absl/types/span.h"
#include "third_party/gloop/util/status/status_macros.h"

namespace security::fft {

enum class Normalization {
  kNone = 0,
  kDefault = 1,
};

// Plan classes for caching array of handlers for full-matrix transforms.

// A plan object for N-dimensional FFTs that stores handlers that will be used
// to run the transform along each axis.
template <typename T, size_t N>
class FftPlan {
 public:
  // Creates an FFT plan for transforms of the given shape. The default
  // normalization, if used, matches scipy's default behavior. Otherwise no
  // normalization is applied.
  explicit FftPlan(const std::array<size_t, N>& shape,
                   Normalization normalization = Normalization::kDefault)
      : FftPlan(shape, /*use_default_normalization=*/
                (normalization == Normalization::kDefault),
                std::make_index_sequence<N>{}) {}

  // Returns the shape as a Span that can be passed to the Rust functions.
  absl::Span<const size_t> shape() const { return absl::MakeConstSpan(shape_); }

  // Returns the underlying Rust handler (FftHandlerF32 or FftHandlerF64) for
  // the i-th axis.
  auto handler(size_t i) const { return handlers_[i].handler(); }

 private:
  template <size_t... Is>
  FftPlan(const std::array<size_t, N>& shape, bool use_default_normalization,
          std::index_sequence<Is...>)
      : shape_(shape),
        handlers_{
            internal::FftHandler<T>(shape[Is], use_default_normalization)...} {}

  std::array<size_t, N> shape_;
  std::array<internal::FftHandler<T>, N> handlers_;
};

// A plan for real-to-complex FFTs.
template <typename T, size_t N>
class R2cFftPlan {
 public:
  // Creates an real-to-complex FFT plan for transforms of the given shape. The
  // default normalization, if used, matches scipy's default behavior. Otherwise
  // no normalization is applied.
  explicit R2cFftPlan(const std::array<size_t, N>& shape,
                      Normalization normalization = Normalization::kDefault)
      : R2cFftPlan(shape, /*use_default_normalization=*/
                   (normalization == Normalization::kDefault),
                   std::make_index_sequence<N - 1>{}) {}

  // Returns the shape for the r2c step.
  absl::Span<const size_t> shape_r2c() const {
    return absl::MakeConstSpan(shape_r2c_);
  }

  // Returns the shape for the c2c steps.
  absl::Span<const size_t> shape_c2c() const {
    return absl::MakeConstSpan(shape_c2c_);
  }

  // Returns the Rust handler for the r2c step.
  auto handler_r2c() const { return handler_r2c_.handler(); }

  // Returns the Rust handler for the i-th c2c step.
  auto handler_c2c(size_t i) const { return handlers_c2c_[i].handler(); }

 private:
  template <size_t... Is>
  R2cFftPlan(const std::array<size_t, N>& shape, bool use_default_normalization,
             std::index_sequence<Is...>)
      : shape_r2c_(shape),
        shape_c2c_(shape),
        handler_r2c_(shape[N - 1], use_default_normalization),
        handlers_c2c_{
            internal::FftHandler<T>(shape[Is], use_default_normalization)...} {
    shape_c2c_[N - 1] = shape_c2c_[N - 1] / 2 + 1;
  }

  // r2c transforms produce a differently-shaped output, so we need to store two
  // shapes and two sets of handlers: one for the initial r2c transform that
  // halves the size of the last dimension, and another one to run c2c
  // transforms in place.
  std::array<size_t, N> shape_r2c_;
  std::array<size_t, N> shape_c2c_;

  internal::R2cFftHandler<T> handler_r2c_;
  std::array<internal::FftHandler<T>, N - 1> handlers_c2c_;
};

// A plan for real-to-real DCTs.
template <typename T, size_t N>
class DctPlan {
 public:
  // Creates an real-to-complex DCT plan for transforms of the given shape. The
  // default normalization, if used, matches scipy's default behavior. Otherwise
  // no normalization is applied.
  explicit DctPlan(const std::array<size_t, N>& shape,
                   Normalization normalization = Normalization::kDefault)
      : DctPlan(shape, /*use_default_normalization=*/
                (normalization == Normalization::kDefault),
                std::make_index_sequence<N>{}) {}

  // Returns the shape as a Span that can be passed to the Rust functions.
  absl::Span<const size_t> shape() const { return absl::MakeConstSpan(shape_); }

  // Returns the underlying Rust handler (FftHandlerF32 or FftHandlerF64) for
  // the i-th axis.
  auto handler(size_t i) const { return handlers_[i].handler(); }

 private:
  template <size_t... Is>
  DctPlan(const std::array<size_t, N>& shape, bool use_default_normalization,
          std::index_sequence<Is...>)
      : shape_(shape),
        handlers_{
            internal::DctHandler<T>(shape[Is], use_default_normalization)...} {}

  std::array<size_t, N> shape_;
  std::array<internal::DctHandler<T>, N> handlers_;
};

// Runs an out-of-place FFT, transforming the data in the `input` buffer and
// storing the results in the `output` buffer. `input` and `output` must be
// non-overlapping buffers of the same size.
template <typename T, size_t N>
absl::Status Fft(ArrayLayout<const std::complex<T>> input,
                 ArrayLayout<std::complex<T>> output,
                 const FftPlan<T, N>& plan) {
  if constexpr (std::is_same_v<T, float>) {
    return internal::RunTransform<N>(rust::fft_f32,
                                     rust::fft_f32_inplace, plan,
                                     input, output);
  } else {
    return internal::RunTransform<N>(rust::fft_f64,
                                     rust::fft_f64_inplace, plan,
                                     input, output);
  }
}

template <typename T, size_t N>
absl::Status Fft(absl::Span<const std::complex<T>> input,
                 absl::Span<std::complex<T>> output,
                 const FftPlan<T, N>& plan) {
  return Fft(ArrayLayout(input, plan.shape(), 1),
             ArrayLayout(output, plan.shape(), 1), plan);
}

// Runs an out-of-place inverse FFT, transforming the data in the `input` buffer
// and storing the results in the `output` buffer. `input` and `output` must be
// non-overlapping buffers of the same size.
template <typename T, size_t N>
absl::Status Ifft(ArrayLayout<const std::complex<T>> input,
                  ArrayLayout<std::complex<T>> output,
                  const FftPlan<T, N>& plan) {
  if constexpr (std::is_same_v<T, float>) {
    return internal::RunTransform<N>(rust::ifft_f32,
                                     rust::ifft_f32_inplace, plan,
                                     input, output);
  } else {
    return internal::RunTransform<N>(rust::ifft_f64,
                                     rust::ifft_f64_inplace, plan,
                                     input, output);
  }
}

template <typename T, size_t N>
absl::Status Ifft(absl::Span<const std::complex<T>> input,
                  absl::Span<std::complex<T>> output,
                  const FftPlan<T, N>& plan) {
  return Ifft(ArrayLayout(input, plan.shape(), 1),
              ArrayLayout(output, plan.shape(), 1), plan);
}

// Runs an out-of-place real-to-complex FFT, transforming the data in the
// `input` buffer and storing the results in the `output` buffer. `input` and
// `output` must be non-overlapping.
//
// The output of a real-to-complex transform has Hermitian symmetry, which makes
// half of the outputs redundant. Because of this, we store only n/2+1 elements
// of the last dimension. So, for example, a transform on a (32, 32, 32) matrix
// of reals would produce a (32, 32, 17) matrix of complex numbers. The `output`
// buffer must be sized accordingly.
//
// See the explanation at
// https://www.fftw.org/fftw3_doc/Real_002ddata-DFT-Array-Format.html.
template <typename T, size_t N>
absl::Status FftR2c(ArrayLayout<const T> input,
                    ArrayLayout<std::complex<T>> output,
                    const R2cFftPlan<T, N>& plan) {
  auto in_slice = internal::ToConstSliceRef(input.data);
  auto out_slice = internal::ToSliceRef(output.data);
  // Run the r2c step on the LAST dimension, then c2c on the others.
  if constexpr (std::is_same_v<T, float>) {
    ABSL_RETURN_IF_ERROR(internal::ToStatus(rust::fft_r2c_f32(
        plan.shape_r2c(), in_slice, input.nembed, input.stride, out_slice,
        output.nembed, output.stride, plan.handler_r2c(), N - 1)));
  } else {
    ABSL_RETURN_IF_ERROR(internal::ToStatus(rust::fft_r2c_f64(
        plan.shape_r2c(), in_slice, input.nembed, input.stride, out_slice,
        output.nembed, output.stride, plan.handler_r2c(), N - 1)));
  }
  for (size_t i = 0; i < N - 1; ++i) {
    if constexpr (std::is_same_v<T, float>) {
      ABSL_RETURN_IF_ERROR(internal::ToStatus(rust::fft_f32_inplace(
          plan.shape_c2c(), out_slice, output.nembed, output.stride,
          plan.handler_c2c(i), i)));
    } else {
      ABSL_RETURN_IF_ERROR(internal::ToStatus(rust::fft_f64_inplace(
          plan.shape_c2c(), out_slice, output.nembed, output.stride,
          plan.handler_c2c(i), i)));
    }
  }
  return absl::OkStatus();
}

template <typename T, size_t N>
absl::Status FftR2c(absl::Span<const T> input,
                    absl::Span<std::complex<T>> output,
                    const R2cFftPlan<T, N>& plan) {
  return FftR2c(ArrayLayout(input, plan.shape_r2c(), 1),
                ArrayLayout(output, plan.shape_c2c(), 1), plan);
}

// Runs an out-of-place complex-to-real inverse FFT, transforming the data in
// the `input` buffer and storing the results in the `output` buffer. `input`
// and `output` must be non-overlapping.
//
// Note that this is complex-to-real despite of having "R2c" in the function
// name. That is, `IfftR2c` is the inverse of `FftR2c`.
//
// `IfftR2c` uses the same plan type as `FftR2c`. This means that in this case
// the dimensions of the plan correspond to the OUTPUT matrix, and it's the
// input that is halved in the last dimension, in the same format produced by
// `FftR2c`.
//
// For N > 1, if you don't need the contents of `input` after computing the
// IFFT, prefer using `IfftR2cUsingInputAsScratch` instead.
template <typename T, size_t N>
absl::Status IfftR2c(ArrayLayout<const std::complex<T>> input,
                     ArrayLayout<T> output, const R2cFftPlan<T, N>& plan) {
  auto out_slice = internal::ToSliceRef(output.data);
  // 1D is a special case because we don't need to start with N-1 complex IFFTs,
  // so we can avoid copying the input to a temp buffer altogether.
  if constexpr (N == 1) {
    if constexpr (std::is_same_v<T, float>) {
      return internal::ToStatus(rust::ifft_r2c_f32(
          plan.shape_r2c(), internal::ToConstSliceRef(input.data), input.nembed,
          input.stride, out_slice, output.nembed, output.stride,
          plan.handler_r2c(), 0));
    } else {
      return internal::ToStatus(rust::ifft_r2c_f64(
          plan.shape_r2c(), internal::ToConstSliceRef(input.data), input.nembed,
          input.stride, out_slice, output.nembed, output.stride,
          plan.handler_r2c(), 0));
    }
  } else {
    // Run the c2c steps first, then c2r in the last axis.
    // We need a scratch buffer to store the result of c2c steps because input
    // is const. To avoid copying the input data (including handling the
    // case where the data is not contiguous in memory), we do a three-step
    // process:
    // 1. Transform axis 0 (C2C) from `input` to `temp`.
    // 2. Run N-2 in-place C2C transforms on `temp`.
    // 3. Transform axis N-1 (C2R) from `temp` to `output`.
    //
    // Note that `temp` is always contiguous, independently of the input layout.

    // Transform axis 0.
    std::vector<std::complex<T>> temp;
    temp.resize(internal::NumElements(plan.shape_c2c()));
    auto temp_slice = internal::ToSliceRef(absl::MakeSpan(temp));
    if constexpr (std::is_same_v<T, float>) {
      ABSL_RETURN_IF_ERROR(internal::ToStatus(rust::ifft_f32(
          plan.shape_c2c(), internal::ToConstSliceRef(input.data), input.nembed,
          input.stride, temp_slice, plan.shape_c2c(), 1, plan.handler_c2c(0),
          0)));
    } else {
      ABSL_RETURN_IF_ERROR(internal::ToStatus(rust::ifft_f64(
          plan.shape_c2c(), internal::ToConstSliceRef(input.data), input.nembed,
          input.stride, temp_slice, plan.shape_c2c(), 1, plan.handler_c2c(0),
          0)));
    }
    // Run the rest of the in-place C2C FFTs.
    for (size_t i = 1; i < N - 1; ++i) {
      if constexpr (std::is_same_v<T, float>) {
        ABSL_RETURN_IF_ERROR(internal::ToStatus(rust::ifft_f32_inplace(
            plan.shape_c2c(), temp_slice, plan.shape_c2c(), 1,
            plan.handler_c2c(i), i)));
      } else {
        ABSL_RETURN_IF_ERROR(internal::ToStatus(rust::ifft_f64_inplace(
            plan.shape_c2c(), temp_slice, plan.shape_c2c(), 1,
            plan.handler_c2c(i), i)));
      }
    }
    // Finally, transform axis N-1 (C2R).
    if constexpr (std::is_same_v<T, float>) {
      ABSL_RETURN_IF_ERROR(internal::ToStatus(rust::ifft_r2c_f32(
          plan.shape_r2c(), internal::ToConstSliceRef<T>(temp),
          plan.shape_c2c(), 1, out_slice, output.nembed, output.stride,
          plan.handler_r2c(), N - 1)));
    } else {
      ABSL_RETURN_IF_ERROR(internal::ToStatus(rust::ifft_r2c_f64(
          plan.shape_r2c(), internal::ToConstSliceRef<T>(temp),
          plan.shape_c2c(), 1, out_slice, output.nembed, output.stride,
          plan.handler_r2c(), N - 1)));
    }
  }
  return absl::OkStatus();
}

template <typename T, size_t N>
absl::Status IfftR2c(absl::Span<const std::complex<T>> input,
                     absl::Span<T> output, const R2cFftPlan<T, N>& plan) {
  return IfftR2c(ArrayLayout(input, plan.shape_c2c(), 1),
                 ArrayLayout(output, plan.shape_r2c(), 1), plan);
}

// Runs an out-of-place complex-to-real inverse FFT, transforming the data in
// the `input` buffer and storing the results in the `output` buffer. It also
// uses `input` as scratch space, replacing its contents with intermediate
// results. `input` and `output` must be non-overlapping.
//
// Note that this is complex-to-real despite of having "R2c" in the function
// name. That is, `IfftR2c` is the inverse of `FftR2c`.
//
// `IfftR2c` uses the same plan type as `FftR2c`. This means that in this case
// the dimensions of the plan correspond to the OUTPUT matrix, and it's the
// input that is halved in the last dimension, in the same format produced by
// `FftR2c`.
template <typename T, size_t N>
absl::Status IfftR2cUsingInputAsScratch(ArrayLayout<std::complex<T>> input,
                                        ArrayLayout<T> output,
                                        const R2cFftPlan<T, N>& plan) {
  auto in_slice = internal::ToSliceRef(input.data);
  auto out_slice = internal::ToSliceRef(output.data);
  // Run the c2c steps first, then c2r in the last axis.
  for (size_t i = 0; i < N - 1; ++i) {
    if constexpr (std::is_same_v<T, float>) {
      ABSL_RETURN_IF_ERROR(internal::ToStatus(rust::ifft_f32_inplace(
          plan.shape_c2c(), in_slice, input.nembed, input.stride,
          plan.handler_c2c(i), i)));
    } else {
      ABSL_RETURN_IF_ERROR(internal::ToStatus(rust::ifft_f64_inplace(
          plan.shape_c2c(), in_slice, input.nembed, input.stride,
          plan.handler_c2c(i), i)));
    }
  }
  if constexpr (std::is_same_v<T, float>) {
    return internal::ToStatus(rust::ifft_r2c_f32(
        plan.shape_r2c(), internal::ToConstSliceRef<T>(input.data),
        input.nembed, input.stride, out_slice, output.nembed, output.stride,
        plan.handler_r2c(), N - 1));
  } else {
    return internal::ToStatus(rust::ifft_r2c_f64(
        plan.shape_r2c(), internal::ToConstSliceRef<T>(input.data),
        input.nembed, input.stride, out_slice, output.nembed, output.stride,
        plan.handler_r2c(), N - 1));
  }
}

template <typename T, size_t N>
absl::Status IfftR2cUsingInputAsScratch(absl::Span<std::complex<T>> input,
                                        absl::Span<T> output,
                                        const R2cFftPlan<T, N>& plan) {
  return IfftR2cUsingInputAsScratch(ArrayLayout(input, plan.shape_c2c(), 1),
                                    ArrayLayout(output, plan.shape_r2c(), 1),
                                    plan);
}

// Runs an out-of-place DCT-I, transforming the data in the `input` buffer and
// storing the results in the `output` buffer. `input` and `output` must be
// non-overlapping buffers of the same size.
template <typename T, size_t N>
absl::Status Dct1(ArrayLayout<const T> input, ArrayLayout<T> output,
                  const DctPlan<T, N>& plan) {
  if constexpr (std::is_same_v<T, float>) {
    return internal::RunTransform<N>(rust::dct1_f32,
                                     rust::dct1_f32_inplace, plan,
                                     input, output);
  } else {
    return internal::RunTransform<N>(rust::dct1_f64,
                                     rust::dct1_f64_inplace, plan,
                                     input, output);
  }
}

template <typename T, size_t N>
absl::Status Dct1(absl::Span<const T> input, absl::Span<T> output,
                  const DctPlan<T, N>& plan) {
  return Dct1(ArrayLayout(input, plan.shape(), 1),
              ArrayLayout(output, plan.shape(), 1), plan);
}

// Runs an out-of-place DCT-II (aka "the DCT"), transforming the data in the
// `input` buffer and storing the results in the `output` buffer. `input` and
// `output` must be non-overlapping buffers of the same size.
template <typename T, size_t N>
absl::Status Dct2(ArrayLayout<const T> input, ArrayLayout<T> output,
                  const DctPlan<T, N>& plan) {
  if constexpr (std::is_same_v<T, float>) {
    return internal::RunTransform<N>(rust::dct2_f32,
                                     rust::dct2_f32_inplace, plan,
                                     input, output);
  } else {
    return internal::RunTransform<N>(rust::dct2_f64,
                                     rust::dct2_f64_inplace, plan,
                                     input, output);
  }
}

template <typename T, size_t N>
absl::Status Dct2(absl::Span<const T> input, absl::Span<T> output,
                  const DctPlan<T, N>& plan) {
  return Dct2(ArrayLayout(input, plan.shape(), 1),
              ArrayLayout(output, plan.shape(), 1), plan);
}

// Runs an out-of-place DCT-III (aka "the IDCT"), transforming the data in the
// `input` buffer and storing the results in the `output` buffer. `input` and
// `output` must be non-overlapping buffers of the same size.
template <typename T, size_t N>
absl::Status Dct3(ArrayLayout<const T> input, ArrayLayout<T> output,
                  const DctPlan<T, N>& plan) {
  if constexpr (std::is_same_v<T, float>) {
    return internal::RunTransform<N>(rust::dct3_f32,
                                     rust::dct3_f32_inplace, plan,
                                     input, output);
  } else {
    return internal::RunTransform<N>(rust::dct3_f64,
                                     rust::dct3_f64_inplace, plan,
                                     input, output);
  }
}

template <typename T, size_t N>
absl::Status Dct3(absl::Span<const T> input, absl::Span<T> output,
                  const DctPlan<T, N>& plan) {
  return Dct3(ArrayLayout(input, plan.shape(), 1),
              ArrayLayout(output, plan.shape(), 1), plan);
}

// Runs an out-of-place DCT-IV, transforming the data in the `input` buffer and
// storing the results in the `output` buffer. `input` and `output` must be
// non-overlapping buffers of the same size.
template <typename T, size_t N>
absl::Status Dct4(ArrayLayout<const T> input, ArrayLayout<T> output,
                  const DctPlan<T, N>& plan) {
  if constexpr (std::is_same_v<T, float>) {
    return internal::RunTransform<N>(rust::dct4_f32,
                                     rust::dct4_f32_inplace, plan,
                                     input, output);
  } else {
    return internal::RunTransform<N>(rust::dct4_f64,
                                     rust::dct4_f64_inplace, plan,
                                     input, output);
  }
}

template <typename T, size_t N>
absl::Status Dct4(absl::Span<const T> input, absl::Span<T> output,
                  const DctPlan<T, N>& plan) {
  return Dct4(ArrayLayout(input, plan.shape(), 1),
              ArrayLayout(output, plan.shape(), 1), plan);
}

}  // namespace security::fft

#endif  // SECURITY_FFT_FFT_H_
