//! Explain floating-point literals with a decimal point but no fractional digits.
use nodex_ir::{Span, reading::ReadingContent};
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    if node.kind() != "float_literal" || node.has_error() { return None; }
    let written = src.text(node);
    let digits = written.strip_suffix('.')?;
    if digits.is_empty() || !digits.as_bytes()[0].is_ascii_digit() || !digits.bytes().all(|b| b.is_ascii_digit() || b == b'_') { return None; }
    let dot = src.index.span(src.file, node.end_byte() - 1, node.end_byte());
    Some((dot, ReadingContent::Detail {
        label: "Floating-point literal with trailing dot".into(),
        text: format!("`{written}` is a floating-point literal: its final dot belongs to the number even though no digits follow it. Appending a zero gives the equivalent spelling `{written}0`. The literal has no explicit type suffix, so Rust can infer `f32` or `f64` from context; if it remains unconstrained, its default is `f64`. The dot here is not field access and is not the `..` range operator. Converting a decimal literal to a binary floating-point type can require rounding."),
        definition: None,
        borrow: false,
    }))
}
