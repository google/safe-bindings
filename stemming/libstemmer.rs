use snowball_stemmer::Stemmer;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::os::raw::c_int;

use isolang::Language;

#[repr(C)]
pub struct sb_stemmer {
    stemmer: Stemmer,
    stem: String,
    // Whenever `stem` is derived, `out_length` is equal to `stem.len()`. But in some cases (e.g.
    // non-UTF8), `stem` cannot be derived, in which case `out_length` captures the length of the
    // input.
    out_length: i32,
}

#[allow(non_camel_case_types)]
pub type sb_symbol = c_char;

include!("generated_language_list.rs");

#[no_mangle]
pub extern "C" fn sb_stemmer_list() -> *const *const c_char {
    return ALGORITHM_NAMES_CSTR.as_ptr();
}

/// Creates a new stemmer for the given `algorithm`.
///
/// `_charenc` is ignored; only UTF-8 input is supported. Returns null if `algorithm` is not valid
/// UTF-8 or does not name a supported stemming algorithm.
///
/// # Safety
///
/// The caller must ensure that:
/// * `algorithm` is non-null, and the byte range from `algorithm` through the first nul terminator
///   (inclusive) lies within a single allocated object, is at most `isize::MAX` bytes in length,
///   and is valid for reads of initialized `c_char` bytes.
/// * The memory in that byte range is not mutated for the duration of this call.
///
/// If these preconditions hold and this function returns a non-null pointer, the returned pointer
/// points to a live, properly aligned, initialized `sb_stemmer` allocated via
/// `Box::into_raw(Box::new(...))`, transferring exclusive ownership to the caller until it is freed
/// by a single call to `sb_stemmer_delete`.
#[no_mangle]
pub unsafe extern "C" fn sb_stemmer_new(
    algorithm: *const c_char,
    _charenc: *const c_char,
) -> *mut sb_stemmer {
    assert!(!algorithm.is_null());
    // SAFETY: `CStr::from_ptr(algorithm)` requires `algorithm` to be non-null, point to a
    // nul-terminated byte range of at most `isize::MAX` initialized `c_char` bytes within a single
    // allocated object, and not be mutated for the lifetime of the returned `&CStr`. The
    // `assert!(!algorithm.is_null())` check immediately above ensures `algorithm` is non-null
    // (`c_char` has alignment 1) with no intervening modification, and this function's `# Safety`
    // contract requires the caller to uphold the remaining allocation, initialization, length, and
    // immutability guarantees for the duration of this call. The resulting `language_c_str` borrow
    // is confined to this call and does not escape.
    let language_c_str: &CStr = unsafe { CStr::from_ptr(algorithm) };
    let Ok(language_slice) = language_c_str.to_str() else {
        return std::ptr::null_mut();
    };
    let language_name: Option<&str> = ALGORITHM_NAMES
        .iter()
        .position(|&name| name == language_slice)
        .and_then(|i| Some(ALGORITHM_NAMES[i]))
        .or_else(|| {
            Language::from_639_1(language_slice)
                .or_else(|| Language::from_639_3(language_slice))
                .and_then(|language| {
                    ALGORITHM_NAMES
                        .iter()
                        .position(|&el| el == language.to_name().to_string().to_lowercase())
                        .and_then(|i| Some(ALGORITHM_NAMES[i]))
                })
        });

    (match language_name {
        Some(language_name) => Box::into_raw(Box::new(sb_stemmer {
            stemmer: Stemmer::create(language_name.to_string()),
            stem: String::new(),
            out_length: 0,
        })),
        None => std::ptr::null(),
    }) as *mut sb_stemmer
}

/// Deletes a stemmer previously allocated by `sb_stemmer_new`.
///
/// If `stemmer` is null, this function is a no-op.
///
/// # Safety
///
/// The caller must ensure that:
/// * `stemmer` is either null or a pointer previously returned by `sb_stemmer_new` that has not yet
///   been passed to `sb_stemmer_delete`.
/// * Exclusive ownership of `stemmer` is transferred to this call, and neither `stemmer` nor any
///   buffer pointer previously returned by `sb_stemmer_stem` on `stemmer` is accessed concurrently
///   with or after this call begins.
///
/// If these preconditions hold and `stemmer` is non-null, this function takes exclusive ownership
/// of the `sb_stemmer` allocation, drops its fields, and deallocates it exactly once.
#[no_mangle]
pub unsafe extern "C" fn sb_stemmer_delete(stemmer: *mut sb_stemmer) {
    if !stemmer.is_null() {
        // SAFETY: `Box::from_raw(stemmer)` requires `stemmer` to be non-null, properly aligned,
        // allocated by the global allocator with `Layout::new::<sb_stemmer>()` via `Box::into_raw`,
        // initialized to a valid `sb_stemmer`, and uniquely owned without aliasing or double-free.
        // The enclosing `if !stemmer.is_null()` check ensures `stemmer` is non-null with no
        // intervening modification. By this function's `# Safety` contract and `sb_stemmer_new`'s
        // postcondition, `stemmer` was allocated via `Box::into_raw(Box::new(sb_stemmer { ... }))`,
        // points to a live, aligned, initialized `sb_stemmer` that has not yet been freed, and
        // transfers exclusive ownership to this call with no concurrent or subsequent access.
        // `drop` then runs `sb_stemmer`'s destructor and deallocates the box exactly once.
        drop(unsafe { Box::from_raw(stemmer) });
    }
}

/// Extracts the stem of a word.
///
/// * `stemmer` - pointer to a stemmer returned by `sb_stemmer_new`.
/// * `word` - pointer to the input UTF-8 byte buffer.
/// * `size` - length in bytes of `word`.
///
/// # Safety
///
/// The caller must ensure that:
/// * `stemmer` is a non-null pointer returned by `sb_stemmer_new` that has not yet been freed by
///   `sb_stemmer_delete`.
/// * Exclusive access to `stemmer` is maintained for the duration of this call, and no pointer
///   previously returned by `sb_stemmer_stem` on the same `stemmer` is read once this call begins.
/// * `size >= 0`.
/// * If `size > 0`, `word` is non-null, is valid for reads of `size` consecutive initialized bytes
///   within a single allocated object, does not alias `stemmer` or any buffer owned by `stemmer`
///   (such as a pointer returned by a prior `sb_stemmer_stem` call on `stemmer`), and is not
///   mutated for the duration of this call. If `size == 0`, `word` may be null.
///
/// Postconditions:
/// * If `word` is valid UTF-8, the returned pointer borrows internal storage of `stemmer` and
///   remains valid for reads of `sb_stemmer_length(stemmer)` initialized bytes (not guaranteed to
///   be nul-terminated) only until the next call to `sb_stemmer_stem` or `sb_stemmer_delete` on
///   `stemmer`; callers must not mutate or free the returned pointer.
/// * If `word` is not valid UTF-8, the input `word` pointer is returned unchanged with
///   `sb_stemmer_length(stemmer) == size`.
#[no_mangle]
pub unsafe extern "C" fn sb_stemmer_stem(
    stemmer: *mut sb_stemmer,
    word: *const sb_symbol,
    size: c_int,
) -> *const sb_symbol {
    let word = if size == 0 && word.is_null() { c"".as_ptr() } else { word };
    assert!(!stemmer.is_null());
    assert!(!word.is_null());
    assert!(size >= 0);

    // SAFETY: Creating `&mut *stemmer` requires `stemmer` to be non-null, aligned for
    // `sb_stemmer`, dereferenceable for `size_of::<sb_stemmer>()` bytes, initialized to a valid
    // `sb_stemmer`, and exclusively accessed for the borrow's lifetime. The
    // `assert!(!stemmer.is_null())` check above ensures `stemmer` is non-null with no intervening
    // modification. By this function's `# Safety` contract and `sb_stemmer_new`'s postcondition,
    // `stemmer` is a live `Box::into_raw` allocation that is properly aligned, dereferenceable,
    // initialized, and exclusively borrowed for the duration of this call, with no reads from
    // prior `sb_stemmer_stem` pointers and no aliasing from `word` (or `c"".as_ptr()`, which
    // resides in static memory). The resulting `&mut sb_stemmer` borrow is confined to this call
    // and ends when the function returns.
    let stemmer = unsafe { &mut *stemmer };

    // SAFETY: `std::slice::from_raw_parts(word as *const u8, size as usize)` requires a non-null,
    // `u8`-aligned pointer valid for reads of `size as usize` consecutive initialized `u8` bytes
    // within a single allocated object of at most `isize::MAX` bytes, without mutation during the
    // slice's lifetime. `assert!(!word.is_null())` and `assert!(size >= 0)` above ensure `word` is
    // non-null (`u8` has alignment 1) and `size as usize <= c_int::MAX as usize <= isize::MAX as
    // usize`, with no intervening modifications to `word` or `size`. When `size == 0`, any
    // non-null `u8` pointer is valid for a zero-length slice; when `size > 0`, this function's
    // `# Safety` contract guarantees `word` is valid for reads of `size` initialized bytes in a
    // single allocation disjoint from `stemmer` (so `&mut *stemmer` above cannot alias it) and is
    // not mutated during this call. `word_slice` is only read before `stemmer.stem` is updated and
    // does not escape this call.
    let word_slice: &[u8] = unsafe { std::slice::from_raw_parts(word as *const u8, size as usize) };
    let word_slice: &str = match std::str::from_utf8(word_slice) {
        Ok(word) => word,
        Err(_) => {
            stemmer.out_length = size;
            return word;
        }
    };

    let stem_cow = stemmer.stemmer.stem(word_slice);
    match stem_cow {
        std::borrow::Cow::Borrowed(b) => {
            stemmer.stem.clear();
            stemmer.stem.push_str(b);
        }
        std::borrow::Cow::Owned(o) => {
            stemmer.stem = o;
        }
    }
    stemmer.out_length = stemmer.stem.len() as i32;
    stemmer.stem.as_ptr() as *const sb_symbol
}

/// Returns the length in bytes of the result of the most recent call to `sb_stemmer_stem` on
/// `stemmer`, or `-1` if `stemmer` is null.
///
/// * `stemmer` - pointer to a stemmer returned by `sb_stemmer_new`, or null.
///
/// # Safety
///
/// The caller must ensure that:
/// * `stemmer` is either null or a live, properly aligned pointer to an initialized `sb_stemmer`
///   previously returned by `sb_stemmer_new` that has not yet been freed by `sb_stemmer_delete`.
/// * If `stemmer` is non-null, `*stemmer` is not mutated concurrently for the duration of this
///   call.
#[no_mangle]
pub unsafe extern "C" fn sb_stemmer_length(stemmer: *mut sb_stemmer) -> c_int {
    if stemmer.is_null() {
        return -1;
    }

    // SAFETY: Reading `(*stemmer).out_length` requires `stemmer` to be non-null, aligned for
    // `sb_stemmer`, and pointing to a live `sb_stemmer` allocation with an initialized `out_length`
    // field that is not concurrently mutated during the read. The `if stemmer.is_null()` check
    // immediately above ensures `stemmer` is non-null with no intervening modification, and this
    // function's `# Safety` contract together with `sb_stemmer_new`'s postcondition guarantees
    // `stemmer` points to a live, aligned, initialized `sb_stemmer` that is not concurrently
    // mutated during this call. This copies the `i32` field by value without retaining any borrow
    // of `*stemmer`.
    return unsafe { (*stemmer).out_length };
}
