use xml::common::{is_name_char, is_name_start_char};

/// Checks whether `name` is a valid W3C XML `NCName` (`Name - (Char* ':' Char*)`), meaning it is
/// non-empty, starts with a `NameStartChar` (other than `':'`), and all subsequent characters are
/// `NameChar` (other than `':'`).
///
/// See https://www.w3.org/TR/xml-names/#ns-qualnames and https://www.w3.org/TR/xml-names/#ns-decl.
pub fn is_valid_ncname(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first != ':' && is_name_start_char(first) && chars.all(|c| c != ':' && is_name_char(c))
}

/// Like [`is_valid_ncname`], but takes the bytes of a name, which are invalid if they are not
/// UTF-8.
/// ASCII names, by far the most common, are checked byte by byte, without decoding characters.
pub fn is_valid_ncname_bytes(name: &[u8]) -> bool {
    if name.is_ascii() {
        // The ASCII part of `NameStartChar` (without ':') and `NameChar` (without ':').
        let Some((&first, rest)) = name.split_first() else {
            return false;
        };
        return (first.is_ascii_alphabetic() || first == b'_')
            && rest.iter().all(|&b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'));
    }
    std::str::from_utf8(name).is_ok_and(is_valid_ncname)
}

#[cfg(test)]
mod tests {
        use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn test_is_valid_ncname() {
        expect_true!(is_valid_ncname("foo"));
        expect_true!(is_valid_ncname("_bar"));
        expect_true!(is_valid_ncname("a1-b.c"));
        expect_true!(is_valid_ncname("dům"));
        expect_true!(is_valid_ncname("你好"));

        expect_false!(is_valid_ncname(""));
        expect_false!(is_valid_ncname(":"));
        expect_false!(is_valid_ncname("1foo"));
        expect_false!(is_valid_ncname("-bar"));
        expect_false!(is_valid_ncname(".baz"));
        expect_false!(is_valid_ncname("foo:bar"));
    }

    #[gtest]
    fn test_is_valid_ncname_bytes_matches_is_valid_ncname_for_ascii() {
        // Every ASCII name of up to 2 bytes, which covers every pair of first and next byte.
        let ascii: Vec<u8> = (0..0x80).collect();
        let mut names: Vec<Vec<u8>> = vec![vec![]];
        names.extend(ascii.iter().map(|&a| vec![a]));
        names.extend(ascii.iter().flat_map(|&a| ascii.iter().map(move |&b| vec![a, b])));
        for name in names {
            let expected = is_valid_ncname(std::str::from_utf8(&name).unwrap());
            expect_eq!(is_valid_ncname_bytes(&name), expected, "{name:?}");
        }
    }

    #[gtest]
    fn test_is_valid_ncname_bytes() {
        expect_true!(is_valid_ncname_bytes(b"a1-b.c_d"));
        expect_true!(is_valid_ncname_bytes("dům".as_bytes()));
        expect_true!(is_valid_ncname_bytes("a\u{B7}".as_bytes()));
        expect_false!(is_valid_ncname_bytes("\u{B7}a".as_bytes()));
        expect_false!(is_valid_ncname_bytes(b"a:b"));
        expect_false!(is_valid_ncname_bytes(b"a\xFF")); // Not UTF-8.
    }
}
