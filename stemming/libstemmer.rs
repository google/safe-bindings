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
    // SAFETY:
    // - Operation: `CStr::from_ptr(algorithm)`.
    // - `assert!(!algorithm.is_null())` ensures `algorithm` is non-null (`align_of::<c_char>() ==
    //   1` is trivially satisfied), with no intervening code modifying `algorithm`.
    // - By this function's `# Safety` contract, the caller guarantees that the byte range from
    //   `algorithm` through the first nul terminator (inclusive) lies within a single allocated
    //   object, is at most `isize::MAX` bytes in length, is valid for reads of initialized `c_char`
    //   bytes, and is not mutated for the duration of this call.
    // - Postcondition: `language_c_str` borrows that byte range only within this call and does not
    //   escape.
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

#[no_mangle]
pub extern "C" fn sb_stemmer_delete(stemmer: *mut sb_stemmer) {
    if !stemmer.is_null() {
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

    // SAFETY:
    // - Operation: creating the mutable reference `&mut *stemmer`.
    // - `assert!(!stemmer.is_null())` ensures `stemmer` is non-null, with no intervening code
    //   modifying `stemmer`.
    // - By this function's `# Safety` contract and `sb_stemmer_new`'s postcondition, `stemmer` was
    //   allocated via `Box::into_raw(Box::new(...))` and has not been freed, so it is properly
    //   aligned for `sb_stemmer`, dereferenceable for `size_of::<sb_stemmer>()` bytes, and points
    //   to an initialized, valid `sb_stemmer` value.
    // - By `# Safety`, the caller guarantees exclusive access to `stemmer` for the duration of this
    //   call, does not read from any prior `sb_stemmer_stem` return pointer once this call begins,
    //   and ensures `word` does not alias `stemmer` or its owned buffers (when `size == 0` and the
    //   caller passed null, `c"".as_ptr()` points to static read-only memory disjoint from the
    //   heap-allocated `Box<sb_stemmer>`).
    // - Postcondition: the mutable borrow `stemmer: &mut sb_stemmer` is confined to this call and
    //   ends when the function returns after taking `stemmer.stem.as_ptr()`.
    let stemmer = unsafe { &mut *stemmer };

    // SAFETY:
    // - Operation: `std::slice::from_raw_parts(word as *const u8, size as usize)`.
    // - `word as *const u8` has alignment 1 (`align_of::<u8>() == 1`) and is guaranteed non-null by
    //   `assert!(!word.is_null())` (when `size == 0` and the caller passed null, `word` was
    //   replaced above with `c"".as_ptr()`, which is non-null).
    // - `assert!(size >= 0)` ensures `0 <= size <= c_int::MAX`, so the byte length `(size as usize)
    //   * size_of::<u8>() == size as usize <= c_int::MAX as usize <= isize::MAX as usize`.
    // - When `size == 0`, any non-null `u8` pointer is valid for reads of 0 bytes without
    //   address-space wraparound.
    // - When `size > 0`, by this function's `# Safety` contract `word` is valid for reads of `size`
    //   consecutive initialized `u8` bytes within a single allocated object (which cannot wrap
    //   around the address space), is disjoint from `stemmer` (so `&mut *stemmer` above does not
    //   alias or invalidate `word`), and is not mutated for the duration of this call.
    // - Postcondition: `word_slice` (and any `Cow::Borrowed` subslice derived from it) is only read
    //   within this call before `stemmer.stem` is updated and does not escape.
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

/// Returns the length of the stem of the last word.
///
/// * `stemmer` - pointer to a stemmer.
///
/// SAFETY: stemmer points to an initialized sb_stemmer.
#[no_mangle]
pub unsafe extern "C" fn sb_stemmer_length(stemmer: *mut sb_stemmer) -> c_int {
    if stemmer.is_null() {
        return -1;
    }

    return unsafe { (*stemmer).out_length };
}
