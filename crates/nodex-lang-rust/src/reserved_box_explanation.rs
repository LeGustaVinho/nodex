//! Explain the reserved `box` keyword, including in invalid syntax.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    let mut stack = vec![container];
    let mut items = Vec::new();
    while let Some(part) = stack.pop() {
        if matches!(part.kind(), "mod_item" | "macro_definition" | "macro_invocation" | "token_tree" | "string_literal" | "raw_string_literal" | "char_literal" | "line_comment" | "block_comment") { continue; }
        let descendants = children(part);
        if descendants.is_empty() && src.text(part) == "box" {
            items.push(ContractItem {
                group: "Reserved word box".into(),
                text: "`box` is a reserved Rust keyword, not an ordinary identifier; `r#box` can be used when an identifier with that spelling is needed. It is distinct from the capitalized name `Box`: if `Box` resolves to the standard library type, `Box::new(value)` creates an owning pointer to a heap-allocated value. The reserved lowercase word does not itself mean `Box::new` in stable Rust. Rust is case-sensitive, and name resolution determines what `Box` refers to. This occurrence is shown even when the surrounding syntax is invalid; strings, comments, and macro token input are not interpreted here as ordinary Rust code.".into(),
                span: src.span(part), type_info: None,
            });
        }
        stack.extend(descendants.into_iter().rev());
    }
    items
}
