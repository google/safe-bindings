#ifndef SECURITY_FFT_FFT_INTERNAL_H_
#define SECURITY_FFT_FFT_INTERNAL_H_

#include <complex>
#include <cstddef>
#include <functional>
#include <numeric>

#include "support/rs_std/slice_ref.h"
#include "array_layout.h"
#include "crubit/rust.h"
#include "absl/status/status.h"
#include "absl/types/span.h"

namespace security::fft::internal {

// Templated wrapper classes for ndrustfft handlers.
//
// Crubit doesn't support generics, so in order to support both f32 and f64 we
// need to expose, FftHandler<T> as FftHandlerF32 and FftHandlerF64. Here we
// undo that and expose a `FftHandler<T>` C++ template, that wraps the
// appropriate Rust type depending on the selected specialization.

template <typename T>
class FftHandler;

template <>
class FftHandler<float> {
 public:
  explicit FftHandler(size_t n, bool use_default_normalization = true)
      : handler_(rust::FftHandlerF32::new_(n).normalization(
            use_default_normalization)) {}

  const rust::FftHandlerF32& handler() const { return handler_; }

 private:
  rust::FftHandlerF32 handler_;
};

template <>
class FftHandler<double> {
 public:
  explicit FftHandler(size_t n, bool use_default_normalization = true)
      : handler_(rust::FftHandlerF64::new_(n).normalization(
            use_default_normalization)) {}

  const rust::FftHandlerF64& handler() const { return handler_; }

 private:
  rust::FftHandlerF64 handler_;
};

template <typename T>
class R2cFftHandler;

template <>
class R2cFftHandler<float> {
 public:
  explicit R2cFftHandler(size_t n, bool use_default_normalization = true)
      : handler_(rust::R2cFftHandlerF32::new_(n).normalization(
            use_default_normalization)) {}

  const rust::R2cFftHandlerF32& handler() const {
    return handler_;
  }

 private:
  rust::R2cFftHandlerF32 handler_;
};

template <>
class R2cFftHandler<double> {
 public:
  explicit R2cFftHandler(size_t n, bool use_default_normalization = true)
      : handler_(rust::R2cFftHandlerF64::new_(n).normalization(
            use_default_normalization)) {}

  const rust::R2cFftHandlerF64& handler() const {
    return handler_;
  }

 private:
  rust::R2cFftHandlerF64 handler_;
};

template <typename T>
class DctHandler;

template <>
class DctHandler<float> {
 public:
  explicit DctHandler(size_t n, bool use_default_normalization = true)
      : handler_(rust::DctHandlerF32::new_(n).normalization(
            use_default_normalization)) {}

  const rust::DctHandlerF32& handler() const { return handler_; }

 private:
  rust::DctHandlerF32 handler_;
};

template <>
class DctHandler<double> {
 public:
  explicit DctHandler(size_t n, bool use_default_normalization = true)
      : handler_(rust::DctHandlerF64::new_(n).normalization(
            use_default_normalization)),
        size_(n) {}

  const rust::DctHandlerF64& handler() const { return handler_; }
  size_t size() const { return size_; }

 private:
  rust::DctHandlerF64 handler_;
  size_t size_;
};

// A traits struct to select the right Rust type for a given C++ numeric type.
//
// Due to some Crubit limitations around generics, we can't use Rust's
// `Complex<float>` or `Complex<double>` in the C++/Rust interface. So we use
// the `ComplexF32` and `ComplexF64` structs as transparent wrappers (plus
// `RealF32` and `RealF64` so we can handle real and complex numbers in a
// uniform way). These wrapper types are layout-compatible with the C++ types
// so we convert between them by using reinterpret_cast at the boundary.
template <typename T>
struct FftTraits {};

template <>
struct FftTraits<float> {
  using Real = rust::RealF32;
  using Complex = rust::ComplexF32;
};

template <>
struct FftTraits<double> {
  using Real = rust::RealF64;
  using Complex = rust::ComplexF64;
};

// Converts a ResultUnit from Rust into an absl::Status.
inline absl::Status ToStatus(const rust::ResultUnit& result) {
  if (result.is_ok()) {
    return absl::OkStatus();
  }
  return absl::InternalError(result.unwrap_err_ref());
}

// Converts a span of real values to a slice of the appropriate Rust wrapper
// type (RealF32 or RealF64).
template <typename T>
rs_std::SliceRef<typename FftTraits<T>::Real> ToSliceRef(absl::Span<T> span) {
  return rs_std::SliceRef<typename FftTraits<T>::Real>(absl::MakeSpan(
      reinterpret_cast<typename FftTraits<T>::Real*>(span.data()),
      span.size()));
}

// Converts a span of complex values to a slice of the appropriate Rust wrapper
// type (ComplexF32 or ComplexF64).
template <typename T>
rs_std::SliceRef<typename FftTraits<T>::Complex> ToSliceRef(
    absl::Span<std::complex<T>> span) {
  return rs_std::SliceRef<typename FftTraits<T>::Complex>(absl::MakeSpan(
      reinterpret_cast<typename FftTraits<T>::Complex*>(span.data()),
      span.size()));
}

// Converts a span of const real values to a const slice of the appropriate Rust
// wrapper type (RealF32 or RealF64).
template <typename T>
rs_std::SliceRef<const typename FftTraits<T>::Real> ToConstSliceRef(
    absl::Span<const T> span) {
  return rs_std::SliceRef<const typename FftTraits<T>::Real>(
      absl::MakeConstSpan(
          reinterpret_cast<const typename FftTraits<T>::Real*>(span.data()),
          span.size()));
}

// Converts a span of const complex values to a const slice of the appropriate
// Rust wrapper type (ComplexF32 or ComplexF64).
template <typename T>
rs_std::SliceRef<const typename FftTraits<T>::Complex> ToConstSliceRef(
    absl::Span<const std::complex<T>> span) {
  return rs_std::SliceRef<const typename FftTraits<T>::Complex>(
      absl::MakeConstSpan(
          reinterpret_cast<const typename FftTraits<T>::Complex*>(span.data()),
          span.size()));
}

// Runs an N-dimensional transform by running `t` from `input` to `output` along
// the first axis, then applying `t_inplace` on `output` on each remaining axis.
template <size_t N, typename PlanType, typename Transform,
          typename TransformInPlace, typename InT, typename OutT>
absl::Status RunTransform(Transform t, TransformInPlace t_inplace,
                          const PlanType& plan, ArrayLayout<const InT> input,
                          ArrayLayout<OutT> output) {
  auto in_slice = ToConstSliceRef(input.data);
  auto out_slice = ToSliceRef(output.data);
  absl::Status status = internal::ToStatus(
      t(plan.shape(), in_slice, input.nembed, input.stride, out_slice,
        output.nembed, output.stride, plan.handler(0), 0));
  for (size_t i = 1; i < N; ++i) {
    if (!status.ok()) {
      return status;
    }
    status =
        internal::ToStatus(t_inplace(plan.shape(), out_slice, output.nembed,
                                     output.stride, plan.handler(i), i));
  }
  return status;
}

// Returns the number of elements in an array of the given shape.
inline size_t NumElements(absl::Span<const size_t> shape) {
  return std::accumulate(shape.begin(), shape.end(), size_t{1},
                         std::multiplies<size_t>());
}

}  // namespace security::fft::internal

#endif  // SECURITY_FFT_FFT_INTERNAL_H_
