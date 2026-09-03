//! XML nesting-depth validation using `xml`.

use xml::reader::{ParserConfig, XmlEvent};

/// Error returned by [`check_nesting_depth`] when validation fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NestingDepthError {
    /// The XML element nesting depth exceeded `max_depth`.
    ExceededMaxDepth,
    /// The XML input could not be parsed as well-formed XML.
    MalformedXml,
}

/// Validates that `xml` is parsed as well-formed by `xml` using `config` and its element
/// nesting depth does not exceed `max_depth`.
pub fn check_nesting_depth(
    xml: &[u8],
    max_depth: usize,
    config: ParserConfig,
) -> Result<(), NestingDepthError> {
    let mut reader = config.create_reader(xml);
    let mut depth: usize = 0;
    loop {
        match reader.next() {
            Ok(XmlEvent::StartElement { .. }) => {
                if depth >= max_depth {
                    return Err(NestingDepthError::ExceededMaxDepth);
                }
                depth += 1;
            }
            Ok(XmlEvent::EndElement { .. }) => {
                let Some(new_depth) = depth.checked_sub(1) else {
                    return Err(NestingDepthError::MalformedXml);
                };
                depth = new_depth;
            }
            Ok(XmlEvent::EndDocument) => {
                return if depth == 0 { Ok(()) } else { Err(NestingDepthError::MalformedXml) };
            }
            Ok(_) => {}
            Err(_) => return Err(NestingDepthError::MalformedXml),
        }
    }
}

#[cfg(test)]
mod tests {
        use super::*;
    use googletest::prelude::*;

    const MISC_NODES_XML: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
        <!-- top comment -->
        <p:root xmlns:p="http://example.com/ns" p:attr="val">
            <?pi target?>
            <![CDATA[<not><a/><tag></not>]]>
            <!-- inner comment -->
            text content
        </p:root>"#;

    fn make_nested_xml(depth: usize) -> Vec<u8> {
        let mut xml = Vec::with_capacity(depth * 7);
        for _ in 0..depth {
            xml.extend_from_slice(b"<a>");
        }
        for _ in 0..depth {
            xml.extend_from_slice(b"</a>");
        }
        xml
    }

    fn encode_utf16le_with_bom(xml: &str) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in xml.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes
    }

    fn encode_utf16be_with_bom(xml: &str) -> Vec<u8> {
        let mut bytes = vec![0xFE, 0xFF];
        for unit in xml.encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        bytes
    }

    macro_rules! check_nesting_depth_tests {
        ($($name:ident: ($xml:expr, $max_depth:expr, $expected:expr),)*) => {
            $(
                #[gtest]
                fn $name() {
                    let xml = $xml;
                    expect_eq!(
                        check_nesting_depth(xml.as_ref(), $max_depth, ParserConfig::new()),
                        $expected
                    );
                }
            )*
        };
    }

    check_nesting_depth_tests! {
        // Boundary limits 0 and 1.
        test_boundary_zero_empty: (b"", 0, Err(NestingDepthError::MalformedXml)),
        test_boundary_zero_comment_only:
            (b"<!-- comment only -->", 0, Err(NestingDepthError::MalformedXml)),
        test_boundary_zero_self_closing_root:
            (b"<root/>", 0, Err(NestingDepthError::ExceededMaxDepth)),
        test_boundary_zero_open_close_root:
            (b"<root></root>", 0, Err(NestingDepthError::ExceededMaxDepth)),
        test_boundary_one_self_closing_root: (b"<root/>", 1, Ok(())),
        test_boundary_one_root_with_text: (b"<root>text</root>", 1, Ok(())),
        test_boundary_one_self_closing_child:
            (b"<root><child/></root>", 1, Err(NestingDepthError::ExceededMaxDepth)),
        test_boundary_one_open_close_child:
            (b"<root><child></child></root>", 1, Err(NestingDepthError::ExceededMaxDepth)),

        // Nested elements and self-closing tags (depth 3).
        test_nested_self_closing_above_limit:
            (b"<a attr=\"1\"><b><c/></b></a>", 2, Err(NestingDepthError::ExceededMaxDepth)),
        test_nested_self_closing_at_limit: (b"<a attr=\"1\"><b><c/></b></a>", 3, Ok(())),
        test_nested_self_closing_below_limit: (b"<a attr=\"1\"><b><c/></b></a>", 4, Ok(())),
        test_nested_open_close_above_limit:
            (b"<a><b><c></c></b></a>", 2, Err(NestingDepthError::ExceededMaxDepth)),
        test_nested_open_close_at_limit: (b"<a><b><c></c></b></a>", 3, Ok(())),
        test_nested_open_close_below_limit: (b"<a><b><c></c></b></a>", 4, Ok(())),

        // Sibling subtrees.
        test_sibling_subtrees_above_limit: (
            b"<root><a><b/></a><c><d><e/></d></c><f/></root>",
            3,
            Err(NestingDepthError::ExceededMaxDepth)
        ),
        test_sibling_subtrees_at_limit:
            (b"<root><a><b/></a><c><d><e/></d></c><f/></root>", 4, Ok(())),
        test_sibling_subtrees_below_limit:
            (b"<root><a><b/></a><c><d><e/></d></c><f/></root>", 5, Ok(())),
        test_wide_siblings_above_limit: (
            b"<root><s1/><s2/><s3/><s4/><s5/></root>",
            1,
            Err(NestingDepthError::ExceededMaxDepth)
        ),
        test_wide_siblings_at_limit:
            (b"<root><s1/><s2/><s3/><s4/><s5/></root>", 2, Ok(())),

        // Multiple roots (allow_multiple_root_elements defaults to true in ParserConfig).
        test_multiple_roots_above_limit:
            (b"<r/><a><b><c/></b></a>", 2, Err(NestingDepthError::ExceededMaxDepth)),
        test_multiple_roots_at_limit: (b"<r/><a><b><c/></b></a>", 3, Ok(())),

        // Namespaces, processing instructions, CDATA, and comments ignored for depth.
        test_namespaces_and_misc_nodes_zero_limit:
            (MISC_NODES_XML, 0, Err(NestingDepthError::ExceededMaxDepth)),
        test_namespaces_and_misc_nodes_one_limit: (MISC_NODES_XML, 1, Ok(())),

        // Prolog and inner-content constructs that could confuse naive scanners.
        test_doctype_quoted_bracket_above_limit: (
            b"<!DOCTYPE root [<!ENTITY e \"]>\">]><root><a/></root>",
            1,
            Err(NestingDepthError::ExceededMaxDepth)
        ),
        test_doctype_quoted_bracket_at_limit:
            (b"<!DOCTYPE root [<!ENTITY e \"]>\">]><root><a/></root>", 2, Ok(())),
        test_doctype_comment_bracket_above_limit: (
            b"<!DOCTYPE root [<!-- ] -->]><root><a/></root>",
            1,
            Err(NestingDepthError::ExceededMaxDepth)
        ),
        test_doctype_comment_bracket_at_limit:
            (b"<!DOCTYPE root [<!-- ] -->]><root><a/></root>", 2, Ok(())),
        test_end_tag_with_space_above_limit: (
            b"<root><x></x ><a><b/></a></root>",
            2,
            Err(NestingDepthError::ExceededMaxDepth)
        ),
        test_end_tag_with_space_at_limit:
            (b"<root><x></x ><a><b/></a></root>", 3, Ok(())),
        test_cdata_with_closing_tags_above_limit: (
            b"<root><![CDATA[</a></a></a>]]><a><b/></a></root>",
            2,
            Err(NestingDepthError::ExceededMaxDepth)
        ),
        test_cdata_with_closing_tags_at_limit:
            (b"<root><![CDATA[</a></a></a>]]><a><b/></a></root>", 3, Ok(())),
        test_attribute_with_greater_than_above_limit: (
            b"<root><x a=\"1>2\"/><a><b/></a></root>",
            2,
            Err(NestingDepthError::ExceededMaxDepth)
        ),
        test_attribute_with_greater_than_at_limit:
            (b"<root><x a=\"1>2\"/><a><b/></a></root>", 3, Ok(())),
        test_processing_instruction_with_closing_tags_above_limit: (
            b"<root><?pi </a></a> ?><a><b/></a></root>",
            2,
            Err(NestingDepthError::ExceededMaxDepth)
        ),
        test_processing_instruction_with_closing_tags_at_limit:
            (b"<root><?pi </a></a> ?><a><b/></a></root>", 3, Ok(())),
        test_char_ref_closing_tag_above_limit: (
            b"<root>&#60;/a&#62;<a><b/></a></root>",
            2,
            Err(NestingDepthError::ExceededMaxDepth)
        ),
        test_char_ref_closing_tag_at_limit:
            (b"<root>&#60;/a&#62;<a><b/></a></root>", 3, Ok(())),

        // Malformed XML inputs return MalformedXml when within max_depth, or ExceededMaxDepth
        // when the nesting limit is exceeded before the syntax error is encountered.
        test_malformed_unclosed_root: (b"<root>", 10, Err(NestingDepthError::MalformedXml)),
        test_malformed_unclosed_child_before_end_root:
            (b"<root><child></root>", 10, Err(NestingDepthError::MalformedXml)),
        test_malformed_unclosed_root_and_child:
            (b"<root><child>", 10, Err(NestingDepthError::MalformedXml)),
        test_malformed_unclosed_exceeding_limit:
            (b"<a><b><c>", 2, Err(NestingDepthError::ExceededMaxDepth)),
        test_malformed_mismatched_end_tag:
            (b"<root></wrong>", 10, Err(NestingDepthError::MalformedXml)),
        test_malformed_overlapping_tags:
            (b"<a><b></a></b>", 10, Err(NestingDepthError::MalformedXml)),
        test_malformed_unmatched_end_tag: (b"</root>", 10, Err(NestingDepthError::MalformedXml)),
        test_malformed_extra_end_tag:
            (b"<root/></root>", 10, Err(NestingDepthError::MalformedXml)),
        test_malformed_extra_end_tag_after_open_close:
            (b"<root></root></root>", 10, Err(NestingDepthError::MalformedXml)),
        test_malformed_leading_end_tag_before_root:
            (b"</root><root/>", 10, Err(NestingDepthError::MalformedXml)),
        test_malformed_incomplete_tag: (b"<root", 10, Err(NestingDepthError::MalformedXml)),
        test_malformed_unclosed_attribute_quote:
            (b"<root attr=\"unclosed>", 10, Err(NestingDepthError::MalformedXml)),
        test_malformed_unescaped_ampersand:
            (b"<root>a & b</root>", 10, Err(NestingDepthError::MalformedXml)),
        test_malformed_doctype_without_name:
            (b"<!DOCTYPE><root><a/></root>", 10, Err(NestingDepthError::MalformedXml)),
        test_malformed_comment_with_double_dash:
            (b"<root><!-- a -- b --><a/></root>", 10, Err(NestingDepthError::MalformedXml)),
        test_malformed_unknown_entity_reference:
            (b"<root>&nbsp;<a/></root>", 10, Err(NestingDepthError::MalformedXml)),

        // Encodings: UTF-8 without BOM, UTF-8 with BOM, UTF-16LE with BOM, UTF-16BE with BOM,
        // and Latin-1 (ISO-8859-1 / latin1) via XML declaration.
        test_encoding_utf8_without_bom_within_limit:
            (b"<a><b>caf\xC3\xA9</b></a>", 2, Ok(())),
        test_encoding_utf8_without_bom_exceeding_limit:
            (b"<a><b>caf\xC3\xA9</b></a>", 1, Err(NestingDepthError::ExceededMaxDepth)),
        test_encoding_utf8_with_bom_within_limit:
            (b"\xEF\xBB\xBF<a><b>caf\xC3\xA9</b></a>", 2, Ok(())),
        test_encoding_utf8_with_bom_exceeding_limit:
            (b"\xEF\xBB\xBF<a><b>caf\xC3\xA9</b></a>", 1, Err(NestingDepthError::ExceededMaxDepth)),
        test_encoding_utf16le_with_bom_within_limit:
            (encode_utf16le_with_bom("<a><b>text</b></a>"), 2, Ok(())),
        test_encoding_utf16le_with_bom_exceeding_limit: (
            encode_utf16le_with_bom("<a><b>text</b></a>"),
            1,
            Err(NestingDepthError::ExceededMaxDepth)
        ),
        test_encoding_utf16be_with_bom_within_limit:
            (encode_utf16be_with_bom("<a><b>text</b></a>"), 2, Ok(())),
        test_encoding_utf16be_with_bom_exceeding_limit: (
            encode_utf16be_with_bom("<a><b>text</b></a>"),
            1,
            Err(NestingDepthError::ExceededMaxDepth)
        ),
        test_encoding_latin1_iso_8859_1_within_limit: (
            b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><a><b>caf\xE9</b></a>",
            2,
            Ok(())
        ),
        test_encoding_latin1_iso_8859_1_exceeding_limit: (
            b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><a><b>caf\xE9</b></a>",
            1,
            Err(NestingDepthError::ExceededMaxDepth)
        ),
        test_encoding_latin1_alias_within_limit: (
            b"<?xml version=\"1.0\" encoding=\"latin1\"?><a><b>caf\xE9</b></a>",
            2,
            Ok(())
        ),
        test_encoding_latin1_alias_exceeding_limit: (
            b"<?xml version=\"1.0\" encoding=\"latin1\"?><a><b>caf\xE9</b></a>",
            1,
            Err(NestingDepthError::ExceededMaxDepth)
        ),
        test_encoding_undeclared_latin1_rejected_as_invalid_utf8:
            (b"<a><b>caf\xE9</b></a>", 2, Err(NestingDepthError::MalformedXml)),
        test_encoding_utf16_declared_without_bom_rejected: (
            b"<?xml version=\"1.0\" encoding=\"UTF-16\"?><r><a/></r>",
            2,
            Err(NestingDepthError::MalformedXml)
        ),

        // Deep nesting around 256.
        test_deep_nesting_above_limit:
            (make_nested_xml(256), 255, Err(NestingDepthError::ExceededMaxDepth)),
        test_deep_nesting_at_limit: (make_nested_xml(256), 256, Ok(())),
        test_deep_nesting_below_limit: (make_nested_xml(256), 257, Ok(())),
    }

    #[gtest]
    fn test_custom_parser_config_extra_entities_respected() {
        let xml = b"<root>&nbsp;<a><b/></a></root>";
        let config = ParserConfig::new().add_entity("nbsp", "\u{00A0}");
        expect_eq!(
            check_nesting_depth(xml, 2, config.clone()),
            Err(NestingDepthError::ExceededMaxDepth)
        );
        expect_eq!(check_nesting_depth(xml, 3, config), Ok(()));
    }

    #[gtest]
    fn test_custom_parser_config_disallow_multiple_roots_respected() {
        let xml = b"<r1/><r2/>";
        let config = ParserConfig::new().allow_multiple_root_elements(false);
        expect_eq!(check_nesting_depth(xml, 2, config), Err(NestingDepthError::MalformedXml));
    }
}
