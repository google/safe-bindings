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

#[no_mangle]
pub extern "C" fn sb_stemmer_new(
    algorithm: *const c_char,
    _charenc: *const c_char,
) -> *mut sb_stemmer {
    assert!(!algorithm.is_null());
    let language_c_str: &CStr = unsafe { CStr::from_ptr(algorithm) };
    let language_slice: &str = language_c_str.to_str().unwrap();
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

/// Extract stem of a word.
///
/// * `stemmer` - pointer to a stemmer.
/// * `word` - pointer to the word.
/// * `size` - bytes length of the word to stem.
///
/// SAFETY: stemmer points to an initialized sb_stemmer, word points to a char
/// buffer, callers need to pass a size that is valid in their address space.
#[no_mangle]
pub unsafe extern "C" fn sb_stemmer_stem(
    stemmer: *mut sb_stemmer,
    word: *const sb_symbol,
    size: c_int,
) -> *const sb_symbol {
    assert!(!stemmer.is_null());
    assert!(!word.is_null());
    assert!(size >= 0);

    let stemmer = unsafe { &mut *stemmer };

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
