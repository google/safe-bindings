// ArrayLayout is used to define non-contiguous memory layouts for transform
// inputs and outputs. It offers two mechanisms to this end:
//
// - stride: Specifies the distance in elements between consecutive elements in
//   the last dimension. For example, if you have image data stored as a 2D
//   array with rows like `[R, G, B, R, G, B, ...]` you could process a single
//   channel by setting `stride = 3`.
//
// - nembed: Specifies the shape of a larger array that the data is embedded
//   in. That is, if we're doing a 2x2 transform on a piece of a 4x4 array, we
//   need to set `nembed = {4, 4}`. The size of the actual inputs and outputs
//   is always determined by the transform plan.
//
// To use the whole buffer, set `nembed` to the same shape as the transform
// plan and `stride` to 1.
//
// This way to specify layouts is borrowed from FFTW, in order to ease
// migration of existing code. It is equivalent to having a stride for each
// dimension:
//
// - `stride` tells us how many elements to advance in the last dimension.
// - The inner dimensions of `nembed` tell us how many elements to advance
//   to get to the next row, and so on.
// - The outermost dimension of `nembed` is not relevant for index computations,
//   it only determines the total size.
//
// NOTE(b/457854911): consider adding an alternative constructor that takes a
// span of strides, like the one the underlying Rust ArrayView uses. This might
// be useful for new code that doesn't need FFTW-like semantics.

#ifndef SECURITY_FFT_ARRAY_LAYOUT_H_
#define SECURITY_FFT_ARRAY_LAYOUT_H_

#include <cstddef>

#include "absl/base/attributes.h"
#include "absl/log/check.h"
#include "absl/types/span.h"

namespace security::fft {

template <typename T>
struct ArrayLayout {
  // `data` and `nembed` are stored as non-owning `absl::Span` views; both
  // underlying buffers must outlive this `ArrayLayout` and every transform
  // called with it. Both parameters are `ABSL_ATTRIBUTE_LIFETIME_BOUND`, so
  // Clang rejects the common mistake of binding a temporary at the
  // construction site:
  //
  //   ArrayLayout layout(absl::MakeSpan(buf), {4, 4}, 1);  // error: dangling
  //
  // Pass named containers instead:
  //
  //   std::array<size_t, 2> nembed = {4, 4};
  //   ArrayLayout layout(absl::MakeSpan(buf), absl::MakeSpan(nembed), 1);
  //
  // The attribute only catches temporaries bound directly at the construction
  // site. A span whose backing store dies later (for example one created in a
  // scope that has since exited) remains the caller's responsibility.
  //
  // WARNING: `stride` must be positive (> 0). A zero stride causes multiple
  // indices to alias the exact same memory element, violating uniqueness
  // and causing undefined behavior during transforms.
  explicit ArrayLayout(absl::Span<T> data ABSL_ATTRIBUTE_LIFETIME_BOUND,
                       absl::Span<const size_t> nembed
                           ABSL_ATTRIBUTE_LIFETIME_BOUND,
                       size_t stride)
      : data(data), nembed(nembed), stride(stride) {
    CHECK_GT(stride, 0);
  }

  absl::Span<T> data;
  absl::Span<const size_t> nembed;
  size_t stride;
};

// Deduction guide for `ArrayLayout`.
template <typename T>
ArrayLayout(absl::Span<T> data, absl::Span<const size_t> nembed, size_t stride)
    -> ArrayLayout<T>;

}  // namespace security::fft

#endif  // SECURITY_FFT_ARRAY_LAYOUT_H_
