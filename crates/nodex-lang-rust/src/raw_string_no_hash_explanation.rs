//! Explain raw string literals without hash delimiters.
use nodex_ir::{Span, reading::ReadingContent};
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    if node.kind() != "raw_string_literal" || node.has_error() || !src.text(node).starts_with("r\"") { return None; }
    let start = node.start_byte();
    let prefix = src.index.span(src.file, start, start + 1);
    Some((prefix, ReadingContent::Detail {
        label: "Raw string without hashes".into(),
        text: "The `r` prefix makes this a raw string literal: backslashes are ordinary characters and escape sequences are not interpreted. For example, `r\"hello\\n\"` contains a backslash followed by `n`, not a newline; an actual newline may be written directly. This form begins with `r\"` and ends at the next double quote, so it cannot contain a double quote inside without changing the delimiters. A raw string with matching `#` delimiters can contain quotes. The result is a borrowed UTF-8 string slice (`&'static str`), rather than an owned `String`.".into(),
        definition: None, borrow: false,
    }))
}
