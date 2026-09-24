#ifndef SECURITY_LZMA_LZMA_H_
#define SECURITY_LZMA_LZMA_H_

// If LZMA_H is already defined, original lzma is used in the unit. We cannot
// safely mix standard C structs and Rust's implementation.
#ifndef LZMA_H

#include "lzma_rust_api.h"  // IWYU pragma: export

#else
#error \
    "We should not use liblzma and lzma-rust2 within the same compilation unit!"
#endif  /* LZMA_H */
#endif  // SECURITY_LZMA_LZMA_H_
