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
}
