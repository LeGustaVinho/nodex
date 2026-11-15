//! Explain selected malformed numeric tokens recovered as adjacent syntax nodes.
use nodex_ir::{Span, reading::ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// The parser splits these invalid numbers into a literal and a directly
/// adjacent ERROR. Inspect siblings, not arbitrary text, so strings, comments
/// and a separated subsequent expression cannot be mistaken for the token.
pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Vec<(Span, ReadingContent)> {
    // Tree-sitter marks recovered ERROR siblings as extras, so retain those
    // while ignoring actual extra comments and whitespace.
    let parts: Vec<_> = named_children(node).into_iter().filter(|part| !part.is_extra() || part.is_error()).collect();
    let mut result = Vec::new();
    for pair in parts.windows(2) {
        let [prefix, error] = pair else { continue; };
        if !matches!(prefix.kind(), "integer_literal" | "float_literal") || !error.is_error() || prefix.end_byte() != error.start_byte() { continue; }
        let Some(written) = src.text.get(prefix.start_byte()..error.end_byte()) else { continue; };
        let problem = match written {
            "0b0102" => "0b starts a binary integer, whose digits may only be 0 or 1; the digit 2 is not allowed. Choose the intended base or replace the invalid digit.",
            "0o1279" => "0o starts an octal integer, whose digits may only be 0 through 7; the digit 9 is not allowed. Choose the intended base or replace the invalid digit.",
            "2e" => "e starts a decimal exponent, but it is missing exponent digits. For example, 2e3 has an exponent of 3; use the number actually intended.",
            _ => continue,
        };
        result.push((src.index.span(src.file, prefix.start_byte(), error.end_byte()), ReadingContent::Detail {
            label: "Invalid numeric literal".into(),
            text: format!("{written} is not a valid Rust numeric literal. {problem} The syntax parser recovered part of the source, but the Rust compiler rejects this spelling. This explanation identifies this specific problem; it is not complete validation of the surrounding program."),
            definition: None, borrow: false,
        }));
    }
    result
}
