//! Explain a UTF-8 raw string delimited by exactly one hash on each side.
use nodex_ir::{reading::ReadingContent, Span};
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    if node.kind() != "raw_string_literal" || node.has_error() { return None; }
    let written = src.text(node);
    // Do not conflate zero/multiple-hash raw strings or raw byte/C strings.
    if !written.starts_with("r#\"") || !written.ends_with("\"#") { return None; }
    let opening = src.index.span(src.file, node.start_byte(), node.start_byte() + 3);
    Some((opening, ReadingContent::Detail {
        label: "One-hash raw string".into(),
        text: "This raw string begins with `r#\"` and ends with `\"#`. The matching hash lets ordinary double quotes appear inside the text without escaping them; a quote immediately followed by the closing hash ends the literal. Backslashes are literal characters here, so sequences such as `\\n` are not interpreted as newline escapes. A raw string literal has type `&'static str`: it is a shared reference to UTF-8 text stored for the program's duration.".into(),
        definition: None,
        borrow: false,
    }))
}
