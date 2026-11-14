//! Explain un-hashed raw byte string literals.
use nodex_ir::{Span, reading::ReadingContent};
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    if node.kind() != "raw_string_literal" || node.has_error() || !src.text(node).starts_with("br\"") { return None; }
    let prefix = src.index.span(src.file, node.start_byte(), node.start_byte() + 2);
    Some((prefix, ReadingContent::Detail {
        label: "Raw byte string literal".into(),
        text: "The `b` prefix makes this a byte string and `r` makes its contents raw: backslashes are literal bytes, so `\\n` represents a backslash followed by the letter n, not a newline. Raw byte string contents must be ASCII. The literal has type `&'static [u8; N]`, a shared reference to a fixed-size byte array whose length N is determined by the contents. It does not append a zero byte and does not allocate a growable String.".into(),
        definition: None,
        borrow: false,
    }))
}
