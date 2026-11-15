//! Explain the `raw` option in a possible inline-assembly invocation.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return Vec::new(); }
    let Some(path) = node.child_by_field_name("macro") else { return Vec::new(); };
    let name = src.norm(path).replace(char::is_whitespace, "");
    if name.rsplit("::").next() != Some("asm") { return Vec::new(); }
    let Some(tree) = named_children(node).into_iter().find(|part| part.kind() == "token_tree") else { return Vec::new(); };
    if tree.has_error() { return Vec::new(); }
    let tokens: Vec<_> = children(tree).into_iter().filter(|part| !part.is_extra()).collect();
    if tokens.len() < 2 { return Vec::new(); }
    let mut result = Vec::new();
    for segment in tokens[1..tokens.len()-1].split(|part| part.kind() == ",").skip(1) {
        let [options, group] = segment else { continue; };
        if src.text(*options) != "options" || group.kind() != "token_tree" || group.has_error() { continue; }
        let parts: Vec<_> = children(*group).into_iter().filter(|part| !part.is_extra()).collect();
        if parts.len() < 2 || parts[0].kind() != "(" || parts.last().unwrap().kind() != ")" { continue; }
        for entry in parts[1..parts.len()-1].split(|part| part.kind() == ",") {
            let [raw] = entry else { continue; };
            if raw.kind() != "identifier" || src.text(*raw) != "raw" { continue; }
            result.push((*raw, ReadingContent::Detail {
                label: "Raw assembly template".into(),
                text: "If this invocation resolves to Rust's standard `asm!`, `options(raw)` treats the assembly template strings as raw assembly text: braces are not interpreted as operand placeholders, so they need not be escaped as template syntax. This option changes template parsing; it does not make the instructions safe, verify them, or change their target-specific meaning. The written macro name alone does not establish that it resolves to the standard macro.".into(),
                definition: None, borrow: false,
            }));
        }
    }
    result
}
