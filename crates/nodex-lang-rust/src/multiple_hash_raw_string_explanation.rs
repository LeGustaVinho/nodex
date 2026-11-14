//! Explain raw string literals delimited by two or more hashes.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<(nodex_ir::Span, ReadingContent)> {
    if node.kind() != "raw_string_literal" || node.has_error() { return None; }
    let text = src.text(node);
    let rest = text.strip_prefix('r')?;
    let hashes = rest.bytes().take_while(|byte| *byte == b'#').count();
    if hashes < 2 || rest.as_bytes().get(hashes) != Some(&b'"') { return None; }
    let end = node.start_byte() + 2 + hashes;
    let opener = src.index.span(src.file, node.start_byte(), end);
    Some((opener, ReadingContent::Detail {
        label: "Raw string with multiple hashes".into(),
        text: format!("This raw string starts with `r`, followed by {hashes} hash characters and a double quote. Its closing delimiter is a double quote followed by {hashes} hash characters. A quote followed by fewer than {hashes} hashes can therefore appear inside the text without ending the literal. The delimiter is not part of the value, and backslash escape sequences are not interpreted: for example, `\\n` is a backslash and the letter n. A raw string literal produces a borrowed UTF-8 string slice (`&'static str`)."),
        definition: None,
        borrow: false,
    }))
}
