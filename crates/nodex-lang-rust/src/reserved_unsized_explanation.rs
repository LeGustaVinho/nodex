//! Surface reserved `unsized` in recovered syntax without examining macro input.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
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
        if matches!(part.kind(), "identifier" | "type_identifier" | "field_identifier" | "shorthand_field_identifier") && src.text(part) == "unsized" {
            items.push(ContractItem {
                group: "Reserved unsized word".into(),
                text: "`unsized` is a reserved Rust keyword: it currently has no language behavior and cannot be used as an ordinary identifier or modifier. In an identifier position, `r#unsized` is a raw identifier, not a keyword that changes type layout. This reserved word is distinct from dynamically sized types, such as slices or `str`: those types exist and follow their own rules without using an `unsized` keyword. This occurrence is surfaced even when surrounding Rust syntax is invalid; compilation still checks the full program. Text inside strings, comments, and macro token input is not interpreted here as an ordinary Rust identifier.".into(),
                span: src.span(part), type_info: None,
            });
        }
        stack.extend(named_children(part).into_iter().rev());
    }
    items
}
