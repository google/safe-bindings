#ifndef SECURITY_LZMA_LZMA_H_
#define SECURITY_LZMA_LZMA_H_

// If LZMA_H is already defined, original lzma is used in the unit. We cannot
// safely mix standard C structs and Rust's implementation.
#ifndef LZMA_H

// Abstract wrapper: route to the rust-backed library or the original C library.
#ifdef USE_LZMA_C_FALLBACK
// Fallback: use the original C library headers
#include "third_party/liblzma/lzma.h"  // IWYU pragma: export
#elif defined(RS_PREFIX_SET)
// Rust is active: use the prefixed headers
#include "lzma_prefixed_api.h"  // IWYU pragma: export
#else
// Default to the non-prefixed rust-backed library
#include "lzma_rust_api.h"  // IWYU pragma: export
#endif

#else
#ifndef USE_LZMA_C_FALLBACK
#error \
    "We should not use liblzma and lzma-rust2 within the same compilation unit!"
#endif
#endif  /* LZMA_H */
#endif  // SECURITY_LZMA_LZMA_H_
