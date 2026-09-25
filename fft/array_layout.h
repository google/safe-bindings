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

#include "absl/types/span.h"

namespace security::fft {

template <typename T>
struct ArrayLayout {
  explicit ArrayLayout(absl::Span<T> data, absl::Span<const size_t> nembed,
                       size_t stride)
      : data(data), nembed(nembed), stride(stride) {}

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
