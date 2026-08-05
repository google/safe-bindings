use crate::VecU8;
use regex_syntax::ast::parse::ParserBuilder;
use regex_syntax::ast::print::Printer;
use regex_syntax::ast::{
    Assertion, AssertionKind, Ast, ClassAscii, ClassAsciiKind, ClassBracketed, ClassPerlKind,
    ClassSet, ClassSetItem, Concat, Flag, Flags, FlagsItem, FlagsItemKind, Group, GroupKind,
    Position, Span,
};

/// An error that occurred during regex rewriting.
///
/// We can't directly use a `cc_std::std::string` as the error type in a
/// `Result<Rewriter, E>` because Crubit doesn't support using a bridged type
/// there. Instead, we wrap the error string in an opaque type, and offer a
/// method to get the C++ string.
#[derive(Clone, Debug)]
pub struct RewriteError {
    message: String,
}

impl RewriteError {
    pub fn message(&self) -> VecU8 {
        VecU8::from(self.message.as_bytes())
    }
}

/// A rewriter that holds the parsed AST of a regex pattern and allows
/// applying transformations to it.
#[derive(Clone, Debug)]
pub struct Rewriter {
    ast: Ast,
}

impl Rewriter {
    /// Parses the given `pattern` with the specified options and returns a
    /// `Rewriter` holding the AST.
    pub fn new(
        pattern: &[u8],
        ignore_whitespace: bool,
        octal: bool,
        nest_limit: u32,
    ) -> Result<Self, RewriteError> {
        let result = (|| -> Result<Rewriter, String> {
            let mut builder = ParserBuilder::new();
            builder.ignore_whitespace(ignore_whitespace);
            builder.octal(octal);
            builder.nest_limit(nest_limit);
            let mut parser = builder.build();
            // Will fail if pattern is not valid UTF-8, or if it fails to parse.
            let ast = parser
                .parse(std::str::from_utf8(pattern).map_err(|err| err.to_string())?)
                .map_err(|err| err.to_string())?;
            Ok(Rewriter { ast })
        })();
        result.map_err(|err| RewriteError { message: err })
    }

    /// Rewrites the AST to be more compatible with RE2 by replacing Perl
    /// character classes with their ASCII equivalents.
    pub fn rewrite_for_re2_compat(&mut self, mut unicode: bool) {
        rewrite_ast_for_re2_compat(&mut self.ast, &mut unicode);
    }

    /// Wraps the AST in begin and end anchors (`\A(?:pattern)\z`).
    pub fn add_begin_and_end_anchors(&mut self) {
        let dummy_pos = Position::new(0, 1, 1);
        let dummy_span = Span::new(dummy_pos, dummy_pos);
        let old_ast = std::mem::replace(&mut self.ast, Ast::empty(dummy_span));
        self.ast = add_anchors_to_ast(old_ast);
    }

    /// Prints the AST back to a string.
    pub fn finish(self) -> Result<VecU8, VecU8> {
        let mut printer = Printer::new();
        let mut dst = String::new();
        printer.print(&self.ast, &mut dst).map_err::<VecU8, _>(|err| err.to_string().into())?;
        Ok(VecU8::from(dst.as_bytes()))
    }
}

/// Recursively walks the AST and rewrites parts of it for better compatibility with RE2.
fn rewrite_ast_for_re2_compat(ast: &mut Ast, unicode: &mut bool) {
    match ast {
        Ast::Flags(flags) => {
            if let Some(state) = flags.flags.flag_state(Flag::Unicode) {
                *unicode = state;
            }
        }
        // Perl character classes (like `\w`), are unicode-aware in regex but ASCII-only
        // in RE2. We rewrite them to the equivalent ASCII class (like `[[:word:]]`).
        Ast::ClassPerl(perl) => {
            // `\s` is special because we can't use the POSIX bracketed class `[[:space:]]`:
            //   - In RE2, like Perl, `\s` does not include the vertical tab '\v'.
            //   - In Rust regex, both `\s` and `[[:space:]]` include it.
            // So we need to expand it to the specific set of included characters.
            if perl.kind == ClassPerlKind::Space {
                if let Ok(new_ast) = ParserBuilder::new()
                    .octal(false)
                    .ignore_whitespace(false)
                    .build()
                    .parse(if perl.negated { "[^\\t\\n\\f\\r ]" } else { "[\\t\\n\\f\\r ]" })
                {
                    *ast = new_ast;
                }
            } else if let Some(ascii_kind) = map_perl_kind_to_ascii(&perl.kind) {
                let span = perl.span;
                let negated = perl.negated;
                // ClassAscii is not a valid AST node on its own, we need to wrap it in a
                // bracketed class.
                *ast = Ast::ClassBracketed(Box::new(ClassBracketed {
                    span,
                    negated: false,
                    kind: ClassSet::Item(ClassSetItem::Ascii(ClassAscii {
                        span,
                        kind: ascii_kind,
                        negated,
                    })),
                }));
            }
        }
        // Perl character classes can also appear within a bracketed character class
        // (for example `[a-c\d]` for "digits plus letters a, b, and c").
        Ast::ClassBracketed(bracketed) => {
            rewrite_class_set(&mut bracketed.kind);
        }
        // RE2 word boundaries are also not unicode-aware.
        Ast::Assertion(assertion)
            if (assertion.kind == AssertionKind::WordBoundary
                || assertion.kind == AssertionKind::NotWordBoundary) =>
        {
            let span = assertion.span;
            let old_ast = std::mem::replace(ast, Ast::empty(span));
            *ast = Ast::Group(Box::new(Group {
                span,
                kind: GroupKind::NonCapturing(Flags {
                    span,
                    items: vec![
                        FlagsItem { span, kind: FlagsItemKind::Negation },
                        FlagsItem { span, kind: FlagsItemKind::Flag(Flag::Unicode) },
                    ],
                }),
                ast: Box::new(old_ast),
            }));
        }
        // In RE2 compatibility mode, we rewrite dot (`.`) into
        // `(?:.|(?-u:[\xe0-\xef][\x80-\xbf]{2}|[\xf0-\xf4][\x80-\xbf]{3}))`.
        //
        // In Rust's regex crate in unicode mode, `.` matches any valid Unicode scalar value.
        // However, it will not match invalid UTF-8 byte sequences (such as overlong encodings) or
        // UTF-8 encodings of surrogate code points (U+D800 to U+DFFF). RE2, on the other hand,
        // simplifies the 0x80-0x10FFFF rune range into 2-byte ([\xc2-\xdf][\x80-\xbf]), 3-byte
        // ([\xe0-\xef][\x80-\xbf]{2}), and 4-byte ([\xf0-\xf4][\x80-\xbf]{3}) sequences without
        // validating surrogates or overlong encodings (see `Compiler::Add_80_10ffff()` in
        // //third_party/re2/compile.cc).
        //
        // By pairing the standard Unicode dot (`.`) with raw byte sequences for the 3-byte and
        // 4-byte ranges accepted by RE2, we perfectly match RE2's behavior on surrogates and
        // overlong encodings without incorrectly matching standalone invalid bytes. Furthermore,
        // since newline (`\n`, 0x0A) is an ASCII byte, adding non-ASCII sequences never violates
        // the user's `dot_matches_new_line` setting.
        Ast::Dot(_) => {
            if *unicode
                && let Ok(new_ast) =
                    ParserBuilder::new().octal(false).ignore_whitespace(false).build().parse(
                        "(?:.|(?-u:[\\xe0-\\xef][\\x80-\\xbf]{2}|[\\xf0-\\xf4][\\x80-\\xbf]{3}))",
                    )
            {
                *ast = new_ast;
            }
        }
        Ast::Concat(concat) => {
            for a in &mut concat.asts {
                rewrite_ast_for_re2_compat(a, unicode);
            }
        }
        Ast::Alternation(alt) => {
            for a in &mut alt.asts {
                rewrite_ast_for_re2_compat(a, unicode);
            }
        }
        Ast::Repetition(rep) => {
            rewrite_ast_for_re2_compat(&mut rep.ast, unicode);
        }
        Ast::Group(group) => {
            let old_unicode = *unicode;
            if let Some(flags) = group.flags()
                && let Some(state) = flags.flag_state(Flag::Unicode)
            {
                *unicode = state;
            }
            rewrite_ast_for_re2_compat(&mut group.ast, unicode);
            *unicode = old_unicode;
        }
        _ => {}
    }
}

/// Rewrites a character class set.
fn rewrite_class_set(set: &mut ClassSet) {
    match set {
        ClassSet::Item(item) => rewrite_class_set_item(item),
        ClassSet::BinaryOp(op) => {
            rewrite_class_set(&mut op.lhs);
            rewrite_class_set(&mut op.rhs);
        }
    }
}

/// Rewrites an item within a character class set.
fn rewrite_class_set_item(item: &mut ClassSetItem) {
    match item {
        ClassSetItem::Perl(perl) => {
            // `\s` needs special handling of vertical tabs (see handling of Ast::ClassPerl above).
            if perl.kind == ClassPerlKind::Space {
                if let Ok(Ast::ClassBracketed(ref bracketed)) = ParserBuilder::new()
                    .octal(false)
                    .ignore_whitespace(false)
                    .build()
                    .parse(if perl.negated { "[^\\t\\n\\f\\r ]" } else { "[\\t\\n\\f\\r ]" })
                {
                    // Note that this results in a nested bracketed class. For example, [a-z\s]
                    // becomes [a-z[\t\n\f\r ]]. This is fine because the regex crate supports it.
                    *item = ClassSetItem::Bracketed(bracketed.clone());
                }
            } else if let Some(ascii_kind) = map_perl_kind_to_ascii(&perl.kind) {
                let span = perl.span;
                let negated = perl.negated;
                *item = ClassSetItem::Ascii(ClassAscii { span, kind: ascii_kind, negated });
            }
        }
        ClassSetItem::Bracketed(bracketed) => {
            rewrite_class_set(&mut bracketed.kind);
        }
        ClassSetItem::Union(union) => {
            for i in &mut union.items {
                rewrite_class_set_item(i);
            }
        }
        _ => {}
    }
}

/// Maps a Perl character class kind to its corresponding ASCII character class kind.
fn map_perl_kind_to_ascii(kind: &ClassPerlKind) -> Option<ClassAsciiKind> {
    match kind {
        ClassPerlKind::Digit => Some(ClassAsciiKind::Digit),
        ClassPerlKind::Space => None,
        ClassPerlKind::Word => Some(ClassAsciiKind::Word),
    }
}

/// Puts the given Ast in a non-capturing group, and adds '\A' and '\z' anchors before and after it.
/// We need the non-capturing group because the AST printer is not aware of operator precedence and
/// won't produce output that parses back into the same structure.
///
/// For example:
/// - start with 'a|b'. The AST looks like `Alternation['a', 'b']`.
/// - perform a simple concatenation: `Concat('\A', Alternation(['a', 'b']), '\z')`
/// - print the AST. It will produce '\Aa|b\z'.
/// - parse it back. We end up with `Alternation(['\Aa', 'b\z'])`.
///
/// The main advantage of operating with the AST instead of concatenating the anchors directly to
/// the pattern string is that we don't need to worry about comments and whitespace in verbose mode.
fn add_anchors_to_ast(ast: Ast) -> Ast {
    let dummy_pos = Position::new(0, 1, 1);
    let dummy_span = Span::new(dummy_pos, dummy_pos);

    let start_assertion =
        Ast::Assertion(Box::new(Assertion { span: dummy_span, kind: AssertionKind::StartText }));

    let end_assertion =
        Ast::Assertion(Box::new(Assertion { span: dummy_span, kind: AssertionKind::EndText }));

    let group = Ast::Group(Box::new(Group {
        span: dummy_span,
        kind: GroupKind::NonCapturing(Flags { span: dummy_span, items: Vec::new() }),
        ast: Box::new(ast),
    }));

    Ast::Concat(Box::new(Concat {
        span: dummy_span,
        asts: vec![start_assertion, group, end_assertion],
    }))
}

#[cfg(test)]
mod tests {
        use super::*;
    use googletest::prelude::*;

    fn rewrite(pattern: &str) -> VecU8 {
        let rewriter = Rewriter::new(pattern.as_bytes(), false, false, 250);
        expect_true!(rewriter.is_ok());
        let mut rewriter = rewriter.unwrap();
        rewriter.rewrite_for_re2_compat(true);
        rewriter.finish().unwrap()
    }

    #[gtest]
    fn test_rewrite_perl_digits() {
        expect_eq!(rewrite("\\d"), VecU8::from("[[:digit:]]"));
        expect_eq!(rewrite("\\s"), VecU8::from("[\\t\\n\\f\\r ]"));
        expect_eq!(rewrite("\\w"), VecU8::from("[[:word:]]"));
    }

    #[gtest]
    fn test_rewrite_perl_negated() {
        expect_eq!(rewrite("\\D"), VecU8::from("[[:^digit:]]"));
        expect_eq!(rewrite("\\S"), VecU8::from("[^\\t\\n\\f\\r ]"));
        expect_eq!(rewrite("\\W"), VecU8::from("[[:^word:]]"));
    }

    #[gtest]
    fn test_rewrite_bracketed() {
        expect_eq!(rewrite("[a-z\\d]"), VecU8::from("[a-z[:digit:]]"));
        expect_eq!(rewrite("[^a-z\\d]"), VecU8::from("[^a-z[:digit:]]"));
        expect_eq!(rewrite("[a-z\\s]"), VecU8::from("[a-z[\\t\\n\\f\\r ]]"));
        expect_eq!(rewrite("[^a-z\\s]"), VecU8::from("[^a-z[\\t\\n\\f\\r ]]"));
    }

    #[gtest]
    fn test_rewrite_complex_nested() {
        expect_eq!(
            rewrite("^foo(?:bar[a-z\\s]+|baz(\\d))xyz$"),
            VecU8::from("^foo(?:bar[a-z[\\t\\n\\f\\r ]]+|baz([[:digit:]]))xyz$")
        );
    }

    #[gtest]
    fn test_rewrite_ignore_whitespace() {
        let rewriter = Rewriter::new("\\d \\d".as_bytes(), true, false, 250);
        expect_true!(rewriter.is_ok());
        let mut rewriter = rewriter.unwrap();
        rewriter.rewrite_for_re2_compat(true);
        expect_eq!(rewriter.finish().unwrap(), VecU8::from("[[:digit:]][[:digit:]]"));

        let rewriter = Rewriter::new("\\d \\d".as_bytes(), false, false, 250);
        expect_true!(rewriter.is_ok());
        let mut rewriter = rewriter.unwrap();
        rewriter.rewrite_for_re2_compat(true);
        expect_eq!(rewriter.finish().unwrap(), VecU8::from("[[:digit:]] [[:digit:]]"));
    }

    fn expect_anchored(pattern: &str, expected: &str, ignore_whitespace: bool) {
        let rewriter = Rewriter::new(pattern.as_bytes(), ignore_whitespace, false, 250);
        expect_true!(rewriter.is_ok());
        let mut rewriter = rewriter.unwrap();
        rewriter.add_begin_and_end_anchors();
        expect_eq!(rewriter.finish().unwrap(), VecU8::from(expected));
    }

    #[gtest]
    fn test_add_anchors() {
        expect_anchored("foo", "\\A(?:foo)\\z", false);
        expect_anchored("foo|bar", "\\A(?:foo|bar)\\z", false);
    }

    #[gtest]
    fn test_add_anchors_verbose_with_comment() {
        expect_anchored("foo # comment", "\\A(?:foo)\\z", true);
        expect_anchored("foo # comment", "\\A(?:foo # comment)\\z", false);
    }

    #[gtest]
    fn test_rewrite_word_boundaries() {
        expect_eq!(rewrite("\\b"), VecU8::from("(?-u:\\b)"));
        expect_eq!(rewrite("\\B"), VecU8::from("(?-u:\\B)"));
        expect_eq!(rewrite("\\bfoo\\B"), VecU8::from("(?-u:\\b)foo(?-u:\\B)"));
    }

    #[gtest]
    fn test_rewrite_dot_flags() {
        expect_eq!(
            rewrite("."),
            VecU8::from("(?:.|(?-u:[\\xE0-\\xEF][\\x80-\\xBF]{2}|[\\xF0-\\xF4][\\x80-\\xBF]{3}))")
        );
        expect_eq!(rewrite("(?-u:.)"), VecU8::from("(?-u:.)"));
        expect_eq!(rewrite("(?-u)."), VecU8::from("(?-u)."));
        expect_eq!(
            rewrite("(?-u:.)."),
            VecU8::from(
                "(?-u:.)(?:.|(?-u:[\\xE0-\\xEF][\\x80-\\xBF]{2}|[\\xF0-\\xF4][\\x80-\\xBF]{3}))"
            )
        );
    }
}
