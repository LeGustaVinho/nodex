//! Explain raw byte strings delimited with one or more hash marks.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "raw_string_literal" || node.has_error() { return None; }
    let written = src.text(node);
    let delimiter = written.strip_prefix("br")?;
    let hashes = delimiter.bytes().take_while(|byte| *byte == b'#').count();
    if hashes == 0 || delimiter.as_bytes().get(hashes) != Some(&b'"') { return None; }
    let ending = format!("\"{}", "#".repeat(hashes));
    if !written.ends_with(&ending) || written.len() < 3 + hashes + ending.len() { return None; }
    Some((node, ReadingContent::Detail {
        label: "Raw byte string with hash delimiters".into(),
        text: format!("{} is a raw byte string literal. The b prefix makes it a sequence of bytes, and r makes its contents raw: backslashes are literal bytes rather than starting escapes. The {} hash mark(s) after br are part of the delimiter; the literal ends at a quote followed by the same number of hash marks. Hash marks let quotes appear in the content when they do not complete that closing delimiter. Raw byte string contents are restricted to ASCII. The literal has a static reference to a fixed-size byte array, with its length determined by the bytes in the content; it is not a Unicode string.", written, hashes),
        definition: None,
        borrow: false,
    }))
}
