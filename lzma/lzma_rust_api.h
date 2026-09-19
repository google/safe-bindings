// This file contains a simplified subset of definitions copied directly from
// liblzma's headers to define the C API for the Rust bridge.

#ifndef SECURITY_LZMA_LZMA_RUST_API_H_
#define SECURITY_LZMA_LZMA_RUST_API_H_

#include <stddef.h>
#include <stdint.h>

#define LZMA_VERSION_MAJOR 5
#define LZMA_VERSION_MINOR 8
#define LZMA_VERSION_PATCH 3
#define LZMA_VERSION_STABILITY_ALPHA 0
#define LZMA_VERSION_STABILITY_BETA 1
#define LZMA_VERSION_STABILITY_STABLE 2
#define LZMA_VERSION_STABILITY LZMA_VERSION_STABILITY_STABLE
#ifndef LZMA_VERSION_COMMIT
#define LZMA_VERSION_COMMIT ""
#endif
#define LZMA_VERSION                                                          \
  (LZMA_VERSION_MAJOR * UINT32_C(10000000) +                                  \
   LZMA_VERSION_MINOR * UINT32_C(10000) + LZMA_VERSION_PATCH * UINT32_C(10) + \
   LZMA_VERSION_STABILITY)

#ifdef __cplusplus
extern "C" {
#endif

/* ---- Allocator ---- */

typedef struct {
  void* (*alloc)(void* opaque, size_t nmemb, size_t size);
  void (*free)(void* opaque, void* ptr);
  void* opaque;
} lzma_allocator;

/* ---- Return values ---- */

typedef enum {
  LZMA_OK = 0,
  LZMA_STREAM_END = 1,
  LZMA_NO_CHECK = 2,
  LZMA_UNSUPPORTED_CHECK = 3,
  LZMA_GET_CHECK = 4,
  LZMA_MEM_ERROR = 5,
  LZMA_MEMLIMIT_ERROR = 6,
  LZMA_FORMAT_ERROR = 7,
  LZMA_OPTIONS_ERROR = 8,
  LZMA_DATA_ERROR = 9,
  LZMA_BUF_ERROR = 10,
  LZMA_PROG_ERROR = 11,
  LZMA_SEEK_NEEDED = 12
} lzma_ret;

/* ---- Actions ---- */

typedef enum {
  LZMA_RUN = 0,
  LZMA_SYNC_FLUSH = 1,
  LZMA_FULL_FLUSH = 2,
  LZMA_FINISH = 3,
  LZMA_FULL_BARRIER = 4
} lzma_action;

/* ---- Check types ---- */

typedef enum {
  LZMA_CHECK_NONE = 0,
  LZMA_CHECK_CRC32 = 1,
  LZMA_CHECK_CRC64 = 4,
  LZMA_CHECK_SHA256 = 10
} lzma_check;

#define LZMA_CHECK_ID_MAX 15

/* ---- Reserved enum for padding ---- */

typedef enum { LZMA_RESERVED_ENUM = 0 } lzma_reserved_enum;

/* ---- Boolean type ---- */

typedef unsigned char lzma_bool;

/* ---- Compression modes ---- */

typedef enum { LZMA_MODE_FAST = 1, LZMA_MODE_NORMAL = 2 } lzma_mode;

/* ---- Match finders ---- */

typedef enum {
  LZMA_MF_HC3 = 0x03,
  LZMA_MF_HC4 = 0x04,
  LZMA_MF_BT2 = 0x12,
  LZMA_MF_BT3 = 0x13,
  LZMA_MF_BT4 = 0x14
} lzma_match_finder;

/* ---- Variable-length integer ---- */

typedef uint64_t lzma_vli;
#define LZMA_VLI_UNKNOWN UINT64_MAX
#define LZMA_VLI_MAX 0x7fffffffffffffffU

/* ---- Filter IDs ---- */

#define LZMA_FILTER_LZMA1 0x4000000000000001ULL
#define LZMA_FILTER_LZMA2 0x21ULL
#define LZMA_FILTER_DELTA 0x03ULL
#define LZMA_FILTER_X86 0x04ULL
#define LZMA_FILTER_POWERPC 0x05ULL
#define LZMA_FILTER_IA64 0x06ULL
#define LZMA_FILTER_ARM 0x07ULL
#define LZMA_FILTER_ARMTHUMB 0x08ULL
#define LZMA_FILTER_SPARC 0x09ULL
#define LZMA_FILTER_ARM64 0x0AULL
#define LZMA_FILTER_RISCV 0x0BULL

/* ---- LZMA options ---- */

#define LZMA_DICT_SIZE_MIN 4096U
#define LZMA_DICT_SIZE_DEFAULT (1U << 23)

typedef struct {
  uint32_t dict_size;
  const uint8_t* preset_dict;
  uint32_t preset_dict_size;
  uint32_t lc;
  uint32_t lp;
  uint32_t pb;
  lzma_mode mode;
  uint32_t nice_len;
  lzma_match_finder mf;
  uint32_t depth;

  uint32_t reserved_int1;
  uint32_t reserved_int2;
  uint32_t reserved_int3;
  uint32_t reserved_int4;
  uint32_t reserved_int5;
  uint32_t reserved_int6;
  uint32_t reserved_int7;
  uint32_t reserved_int8;
  lzma_reserved_enum reserved_enum1;
  lzma_reserved_enum reserved_enum2;
  lzma_reserved_enum reserved_enum3;
  lzma_reserved_enum reserved_enum4;
  void* reserved_ptr1;
  void* reserved_ptr2;
} lzma_options_lzma;

/* ---- Filter chain ---- */

#define LZMA_FILTERS_MAX 4

typedef struct {
  lzma_vli id;
  void* options;
} lzma_filter;

#define LZMA_LC_DEFAULT 3
#define LZMA_LP_DEFAULT 0
#define LZMA_PB_DEFAULT 2

/* ---- Preset flags ---- */

#define LZMA_PRESET_DEFAULT 6U
#define LZMA_PRESET_EXTREME 0x80000000U
#define LZMA_PRESET_LEVEL_MASK 0x1FU

/* ---- Delta options ---- */

typedef enum { LZMA_DELTA_TYPE_BYTE = 0 } lzma_delta_type;

typedef struct {
  lzma_delta_type type;
  uint32_t dist;
  uint32_t reserved_int1;
  uint32_t reserved_int2;
  uint32_t reserved_int3;
  uint32_t reserved_int4;
  void* reserved_ptr1;
  void* reserved_ptr2;
} lzma_options_delta;

typedef struct {
  uint32_t start_offset;
} lzma_options_bcj;

/* ---- Multithreaded encoder options ---- */

typedef struct {
  uint32_t flags;
  uint32_t threads;
  uint64_t block_size;
  uint32_t timeout;
  uint32_t preset;
  const lzma_filter* filters;
  lzma_check check;

  lzma_reserved_enum reserved_enum1;
  lzma_reserved_enum reserved_enum2;
  lzma_reserved_enum reserved_enum3;
  void* reserved_ptr1;
  void* reserved_ptr2;
  void* reserved_ptr3;
  void* reserved_ptr4;
  uint64_t reserved_int1;
  uint64_t reserved_int2;
  uint64_t reserved_int3;
  uint64_t reserved_int4;
  uint64_t memlimit_threading;
  uint64_t memlimit_stop;
  uint64_t reserved_int7;
  uint64_t reserved_int8;
} lzma_mt;

/* ---- Decoder flags ---- */

#define LZMA_TELL_NO_CHECK 0x01U
#define LZMA_TELL_UNSUPPORTED_CHECK 0x02U
#define LZMA_TELL_ANY_CHECK 0x04U
#define LZMA_CONCATENATED 0x08U
#define LZMA_IGNORE_CHECK 0x10U
#define LZMA_FAIL_FAST 0x20U

/* Mask of all supported decoder flags */
#define LZMA_SUPPORTED_FLAGS                                                \
  (LZMA_TELL_NO_CHECK | LZMA_TELL_UNSUPPORTED_CHECK | LZMA_TELL_ANY_CHECK | \
   LZMA_CONCATENATED | LZMA_IGNORE_CHECK | LZMA_FAIL_FAST)

/* ---- Stream structure ---- */

// Internal state of the stream.
//
// This struct is opaque to the user, and is mapped to Rust's
// `lzma_rust::InternalState` struct.
typedef struct lzma_internal_s lzma_internal;

typedef struct {
  const uint8_t* next_in;
  size_t avail_in;
  uint64_t total_in;

  uint8_t* next_out;
  size_t avail_out;
  uint64_t total_out;

  const void* allocator; /* Accepted but ignored. */
  lzma_internal* internal;

  void* reserved_ptr1;
  void* reserved_ptr2;
  void* reserved_ptr3;
  void* reserved_ptr4;

  uint64_t seek_pos;
  uint64_t reserved_int2;
  size_t reserved_int3;
  size_t reserved_int4;

  lzma_reserved_enum reserved_enum1;
  lzma_reserved_enum reserved_enum2;
} lzma_stream;

/**
 * \brief       Initialization for lzma_stream
 *
 * \note        lzma_stream variables MUST be initialized to LZMA_STREAM_INIT
 *              or zeroed (e.g. with memset()) before they are passed to any
 *              initialization functions. This strictly follows the behavior
 *              required by the original C liblzma implementation.
 */
#define LZMA_STREAM_INIT \
  {NULL,                 \
   0,                    \
   0,                    \
   NULL,                 \
   0,                    \
   0,                    \
   NULL,                 \
   NULL,                 \
   NULL,                 \
   NULL,                 \
   NULL,                 \
   NULL,                 \
   0,                    \
   0,                    \
   0,                    \
   0,                    \
   LZMA_RESERVED_ENUM,   \
   LZMA_RESERVED_ENUM}

/* ---- Decoder functions ---- */

// Initializes a `.lz` (lzip) stream decoder.
//
// `strm` must point to an `lzma_stream` initialized with LZMA_STREAM_INIT.
//
// `memlimit` sets the memory usage limit as bytes.
//
// `flags` is a bitwise-OR of zero or more of the decoder flags. Note that
// unlike standard liblzma, which only decodes concatenated .lz files if
// `LZMA_CONCATENATED` is set and otherwise stops after the first member,
// this implementation always decodes concatenated .lz members by default
// due to underlying `lzma_rust2::LzipStream` behavior. Accepted flags are
// validated, but not acted upon.
// NOTE(b/562256542): match default concatenated behavior of liblzma.
//
// Returns `LZMA_OK` upon successful initialization, `LZMA_OPTIONS_ERROR`
// if unsupported flags are provided, or `LZMA_PROG_ERROR` if state setup fails.
// Unlike standard liblzma, `LZMA_MEM_ERROR` is never returned. Memory
// allocation failures will cause the Rust allocator to panic/abort instead.
lzma_ret lzma_lzip_decoder(lzma_stream* strm, uint64_t memlimit,
                           uint32_t flags);

// Initializes a `.xz` stream decoder.
//
// `strm` must point to an `lzma_stream` initialized with LZMA_STREAM_INIT.
//
// `memlimit` sets the memory usage limit in bytes. Use `UINT64_MAX` to indicate
// no limit.
// `memlimit` is converted to KiB internally, a limit lower than 1024 bytes will
// cause the decoder to fail. This is similar to liblzma.
//
// `flags` is a bitwise-OR of zero or more of the decoder flags. Supported
// flags include `LZMA_CONCATENATED`, `LZMA_TELL_NO_CHECK`, and
// `LZMA_TELL_ANY_CHECK`. Other accepted flags (e.g. `LZMA_IGNORE_CHECK`)
// are silently ignored without returning an error.
//
// Note on processing granularity:
// This implementation buffers a whole LZMA2 chunk before decoding, whereas the
// original liblzma decodes symbol by symbol. Because of this difference, a
// single iteration of `lzma_code` may consume input but not produce any output
// until a full chunk is processed. Callers must handle this by continuing to
// provide input.
//
// Returns `LZMA_OK` upon successful initialization, `LZMA_OPTIONS_ERROR`
// if unsupported flags are provided, or `LZMA_PROG_ERROR`.
// Unlike standard liblzma, `LZMA_MEM_ERROR` is never returned. Memory
// allocation failures will cause the Rust allocator to panic/abort instead.
lzma_ret lzma_stream_decoder(lzma_stream* strm, uint64_t memlimit,
                             uint32_t flags);

// Initializes a legacy LZMA_ALONE (`.lzma`) decoder.
//
// `strm` must point to an `lzma_stream` initialized with LZMA_STREAM_INIT.
//
// `memlimit` sets the memory usage limit in bytes. Use `UINT64_MAX` to indicate
// no limit.
//
// Returns `LZMA_OK` upon successful initialization, or `LZMA_PROG_ERROR` if
// state setup fails. Unlike standard liblzma, `LZMA_MEM_ERROR` is never
// returned. Memory allocation failures will cause the Rust allocator to
// panic/abort instead.
// `memlimit` is converted to KiB internally, a limit lower than 1024 bytes will
// cause the decoder to fail. This is similar to liblzma.
lzma_ret lzma_alone_decoder(lzma_stream* strm, uint64_t memlimit);

// Initializes a raw decoder with a filter chain.
//
// `strm` must point to an `lzma_stream` initialized with LZMA_STREAM_INIT.
//
// `filters` must point to a valid, LZMA_VLI_UNKNOWN-terminated array of
// filters. If any filter's `options` field is non-null, it must point to a
// valid options struct corresponding to its ID (e.g., `lzma_options_lzma` for
// LZMA1/2, `lzma_options_delta` for Delta, `lzma_options_bcj` for BCJ
// filters). Currently, only a single LZMA1 or LZMA2 filter is supported.

// Returns `LZMA_OK` upon successful initialization, `LZMA_OPTIONS_ERROR`
// if the filter chain is unsupported, or `LZMA_PROG_ERROR` if state setup
// fails. Unlike standard liblzma, `LZMA_MEM_ERROR` is never returned. Memory
// allocation failures will cause the Rust allocator to panic/abort instead.
lzma_ret lzma_raw_decoder(lzma_stream* strm, const lzma_filter* filters);

// Initializes a decoder that auto-detects the input stream format.
//
// `memlimit` sets the memory usage limit in bytes. Use `UINT64_MAX` to indicate
// no limit.
// `memlimit` is converted to KiB internally, a limit lower than 1024 bytes will
// cause the decoder to fail. This is similar to liblzma.
//
// `flags` is a bitwise-OR of zero or more of the decoder flags. Note that
// this FFI implementation currently only acts upon the `LZMA_CONCATENATED`,
// `LZMA_TELL_NO_CHECK`, and `LZMA_TELL_ANY_CHECK` flags. Other accepted flags
// are selectively ignored.
//
// Note that when decoding legacy `.lzma` streams, this implementation accepts
// non-standard dictionary sizes that standard liblzma rejects with
// `LZMA_FORMAT_ERROR`.
//
// Returns `LZMA_OK` upon successful initialization, `LZMA_OPTIONS_ERROR`
// if unsupported flags are provided, or `LZMA_PROG_ERROR` if state setup fails.
// Unlike standard liblzma, `LZMA_MEM_ERROR` is never returned. Memory
// allocation failures will cause the Rust allocator to panic/abort instead.
lzma_ret lzma_auto_decoder(lzma_stream* strm, uint64_t memlimit,
                           uint32_t flags);
/* ---- Encoder functions ---- */

// Initializes a `.xz` stream encoder using a preset compression level.
// `preset` specifies the compression level (0-9) and optional flags (like
// `LZMA_PRESET_EXTREME`).
// `check` specifies the integrity check type (e.g., `LZMA_CHECK_CRC32`).
// Returns `LZMA_OK` upon successful initialization, `LZMA_OPTIONS_ERROR`
// if the preset or check type is unsupported, or `LZMA_PROG_ERROR` if state
// setup fails. Unlike standard liblzma, `LZMA_MEM_ERROR` is never returned.
// Memory allocation failures will cause the Rust allocator to panic/abort
// instead.
lzma_ret lzma_easy_encoder(lzma_stream* strm, uint32_t preset,
                           lzma_check check);
// Initializes a `.xz` stream encoder using a custom filter chain.
// `filters` must point to a valid array of `lzma_filter` terminated by a
// filter with `id == LZMA_VLI_UNKNOWN`.
// `check` specifies the integrity check type (e.g., `LZMA_CHECK_CRC32`).
// Returns `LZMA_OK` upon successful initialization, `LZMA_OPTIONS_ERROR`
// if the filter chain or check type is unsupported, or `LZMA_PROG_ERROR` if
// state setup fails.
lzma_ret lzma_stream_encoder(lzma_stream* strm, const lzma_filter* filters,
                             lzma_check check);
// Initializes a `.xz` multithreaded stream encoder using the provided options.
//
// `strm` must point to an `lzma_stream` initialized with LZMA_STREAM_INIT.
//
// `options` must point to a valid `lzma_mt` struct containing the
// initialization options.
//
// Returns `LZMA_OK` upon successful initialization, `LZMA_OPTIONS_ERROR`
// if the options are invalid or unsupported, or `LZMA_PROG_ERROR` if
// state setup fails. Unlike standard liblzma, `LZMA_MEM_ERROR` is never
// returned. Memory allocation failures will cause the Rust allocator to
// panic/abort instead.
lzma_ret lzma_stream_encoder_mt(lzma_stream* strm, const lzma_mt* options);

// Initializes a `.lzma` encoder (legacy LZMA_Alone format) using the provided
// options.
//
// `strm` must point to an `lzma_stream` initialized with LZMA_STREAM_INIT.
//
// `options` must point to a valid `lzma_options_lzma` struct containing the
// initialization options.
//
// Returns `LZMA_OK` upon successful initialization, `LZMA_OPTIONS_ERROR`
// if the options are invalid or unsupported, or `LZMA_PROG_ERROR` if
// state setup fails.
lzma_ret lzma_alone_encoder(lzma_stream* strm,
                            const lzma_options_lzma* options);
// Initializes a raw encoder with a custom filter chain.
//
// `strm` must point to an `lzma_stream` initialized with LZMA_STREAM_INIT.
//
// `filters` must point to a valid array of `lzma_filter` terminated by a
// filter with `id == LZMA_VLI_UNKNOWN`.
//
// Returns `LZMA_OK` upon successful initialization, `LZMA_OPTIONS_ERROR`
// if the filter chain is invalid or unsupported, or `LZMA_PROG_ERROR` if
// state setup fails.
lzma_ret lzma_raw_encoder(lzma_stream* strm, const lzma_filter* filters);
// Initializes a `.lz` (lzip) stream encoder.
//
// `strm` must point to an `lzma_stream` initialized with LZMA_STREAM_INIT.
//
// `preset` specifies the compression level (0-9).
//
// Returns `LZMA_OK` upon successful initialization, `LZMA_OPTIONS_ERROR`
// if the preset is unsupported, or `LZMA_PROG_ERROR` if state setup fails.
// Unlike standard liblzma, `LZMA_MEM_ERROR` is never returned. Memory
// allocation failures will cause the Rust allocator to panic/abort instead.
lzma_ret lzma_lzip_encoder(lzma_stream* strm, uint32_t preset);

/* ---- Processing ---- */

// Encodes or decodes data.
//
// Once the `lzma_stream` has been successfully initialized, actual encoding or
// decoding is done using this function. The application must update
// `strm->next_in`, `strm->avail_in`, `strm->next_out`, and `strm->avail_out`
// to pass input to and receive output from the implementation.
//
// `strm` must point to an `lzma_stream` initialized with LZMA_STREAM_INIT.
//
// `action` describes the action for this function to take. Note that this FFI
// implementation currently supports `LZMA_RUN`, `LZMA_SYNC_FLUSH` (for
// single-threaded encoders), `LZMA_FULL_FLUSH` (for multithreaded encoders),
// and `LZMA_FINISH`. Unsupported combinations return `LZMA_PROG_ERROR`
// or `LZMA_OPTIONS_ERROR`. Passing other actions (like `LZMA_FULL_BARRIER`)
// will return `LZMA_PROG_ERROR`.
//
// Returns an `lzma_ret` enum representing the current state, e.g. `LZMA_OK` if
// progress is made, or `LZMA_STREAM_END` if the stream is finished.
lzma_ret lzma_code(lzma_stream* strm, lzma_action action);

// Frees memory allocated for the coder data structures.
//
// `strm` must point to an `lzma_stream` initialized with LZMA_STREAM_INIT.
void lzma_end(lzma_stream* strm);
/* ---- Filter properties ---- */

// Sets a compression preset to the `lzma_options_lzma` structure.
//
// 0 is the fastest and 9 is the slowest. These match the switches -0 .. -9
// of the xz command line tool. In addition, it is possible to bitwise-or
// flags to the preset. Currently only `LZMA_PRESET_EXTREME` is supported.
//
// `options` is a pointer to the LZMA or LZMA2 options to be filled.
// `preset` is the preset level bitwise-ORed with preset flags.
//
// Returns `true` if the preset is not supported (failure), and `false`
// otherwise (success).
lzma_bool lzma_lzma_preset(lzma_options_lzma* options, uint32_t preset);
// Gets the size of the Filter Properties field.
//
// `size` is a pointer to a `uint32_t` to hold the size of the properties.
// `filter` is the Filter ID and options (the size of the properties may
// vary depending on the options).
//
// Returns `LZMA_OK`, `LZMA_OPTIONS_ERROR`, or `LZMA_PROG_ERROR`.
lzma_ret lzma_properties_size(uint32_t* size, const lzma_filter* filter);

// Encodes the Filter Properties field from `filter` into `props`.
//
// `props` must have room for the property bytes. The required size can be
// determined using `lzma_properties_size`.
lzma_ret lzma_properties_encode(const lzma_filter* filter, uint8_t* props);

// Decodes filter properties from `props` into `filter->options`.
//
// This allocates an options struct (such as lzma_options_lzma or
// lzma_options_delta) depending on the filter type. The caller must free it
// using `free()`. Note that a custom allocator is not supported, and
// `libc::malloc` is always used. The `allocator` argument must be nullptr.
lzma_ret lzma_properties_decode(lzma_filter* filter, const void* allocator,
                                const uint8_t* props, size_t props_size);

/* ---- Utility functions ---- */

const char* lzma_version_string(void);
uint32_t lzma_cputhreads(void);
lzma_bool lzma_check_is_supported(lzma_check check);
uint32_t lzma_crc32(const uint8_t* buf, size_t size, uint32_t crc);
uint64_t lzma_crc64(const uint8_t* buf, size_t size, uint64_t crc);

// Gets the type of the integrity check.
//
// Returns the check ID inside the `lzma_stream`, or `LZMA_CHECK_NONE` if
// called before the stream header has been parsed, if `strm` is nullptr, or if
// the stream lacks an internal state.
lzma_check lzma_get_check(const lzma_stream* strm);

// Gets the combined memory usage limit of all the state components.
//
// For encoders and raw decoders where options are explicitly configured at
// initialization, returns the calculated memory usage in bytes.
//
// Note: For streaming decoders where dictionary size is determined dynamically
// from the stream header (e.g., `lzma_stream_decoder`, `lzma_alone_decoder`,
// `lzma_auto_decoder`, `lzma_lzip_decoder`), this implementation returns 0
// as the underlying Rust decoders do not expose runtime memory usage.
uint64_t lzma_memusage(const lzma_stream* strm);

#ifdef __cplusplus
}
#endif

#endif  // SECURITY_LZMA_LZMA_RUST_API_H_
