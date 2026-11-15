//! Explain line-continuation escapes in ordinary Rust string literals.
//! Rust Reference: https://doc.rust-lang.org/reference/expressions/literal-expr.html#string-continuation-escapes
use nodex_ir::{Span, reading::ReadingContent};
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Vec<(Span, ReadingContent)> {
    if node.kind() != "string_literal" || node.has_error() || !src.text(node).starts_with('"') { return Vec::new(); }
    let bytes = src.text(node).as_bytes();
    let mut i = 1;
    let mut output = Vec::new();
    while i + 1 < bytes.len() {
        if bytes[i] != b'\\' { i += 1; continue; }
        let newline = if bytes.get(i + 1) == Some(&b'\n') { 1 }
            else if bytes.get(i + 1..i + 3) == Some(&b"\r\n"[..]) { 2 }
            else { i += 2; continue; };
        let start = i;
        i += 1 + newline;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n') { i += 1; }
        output.push((src.index.span(src.file, node.start_byte() + start, node.start_byte() + i), ReadingContent::Detail {
            label: "String line continuation".into(),
            text: "In an ordinary string literal, an unescaped backslash immediately followed by a source line break is a string-continuation escape. It contributes no characters to the string: Rust also skips following ASCII spaces, tabs, carriage returns and line feeds, including additional line breaks, until the next non-whitespace character. For example, `first` followed by this escape and then `second` produces `firstsecond`. This differs from `\\n`, which represents a newline character, and from raw strings, which do not process escapes.".into(),
            definition: None,
            borrow: false,
        }));
    }
    output
}
