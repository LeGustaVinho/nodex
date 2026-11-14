//! Explain a Rust integer literal with the `usize` type suffix.
use nodex_ir::{Span, reading::ReadingContent};
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    if node.kind() != "integer_literal" || node.has_error() { return None; }
    let written = src.text(node);
    let digits = written.strip_suffix("usize")?.trim_end_matches('_');
    if digits.is_empty() { return None; }
    let suffix = src.index.span(src.file, node.end_byte() - "usize".len(), node.end_byte());
    Some((suffix, ReadingContent::Detail {
        label: "Pointer-sized integer literal".into(),
        text: format!("In `{written}`, `{digits}` is the integer value and the `usize` suffix explicitly gives the literal Rust's unsigned pointer-sized integer type. Its bit width follows the compilation target's pointer width (commonly 32 or 64 bits), not necessarily the computer running Nodex. The value must fit that target's `usize`; otherwise compilation fails. This type is commonly used for collection lengths and indexes. An underscore before the suffix is an optional separator."),
        definition: None,
        borrow: false,
    }))
}
