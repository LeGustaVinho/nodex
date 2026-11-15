//! Explain standalone empty statements on the containing function's contract.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_item" { return Vec::new(); }
    let Some(body) = node.child_by_field_name("body") else { return Vec::new(); };
    let mut pending = vec![body];
    let mut items = Vec::new();
    while let Some(part) = pending.pop() {
        if part.is_extra() || part.is_error() || matches!(part.kind(), "token_tree" | "closure_expression" | "async_block" | "function_item" | "string_literal" | "raw_string_literal" | "char_literal") { continue; }
        if part.kind() == "empty_statement" && src.text(part) == ";" {
            items.push(ContractItem {
                group: "Empty statement".into(),
                text: "This standalone ; is an empty statement: reaching it performs no computation and does not create a value or binding. It differs from the semicolon that terminates a let declaration or an expression statement. The surrounding control flow continues as it otherwise would.".into(),
                span: src.span(part), type_info: None,
            });
            continue;
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    items
}
