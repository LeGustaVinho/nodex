//! Explain how grouped addition/subtraction binds inside multiplication.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn describe<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, String)> {
    if node.kind() != "binary_expression" || node.has_error() { return None; }
    if !children(node).into_iter().any(|part| part.kind() == "*") { return None; }
    let left = node.child_by_field_name("left")?;
    let right = node.child_by_field_name("right")?;
    let (group, other) = if left.kind() == "parenthesized_expression" { (left, right) }
        else if right.kind() == "parenthesized_expression" { (right, left) } else { return None; };
    let parts: Vec<_> = named_children(group).into_iter().filter(|part| !part.is_extra()).collect();
    let [inner] = parts.as_slice() else { return None; };
    if inner.kind() != "binary_expression" { return None; }
    let operator = children(*inner).into_iter().find(|part| matches!(part.kind(), "+" | "-"))?;
    let operation = if operator.kind() == "+" { "addition" } else { "subtraction" };
    Some((group, format!("In `{}`, the parentheses group the {} `{}` as one operand of the multiplication with `{}`. Without these parentheses, multiplication binds more tightly than addition and subtraction, so the unparenthesized expression can have a different grouping. Parentheses describe which operands belong to each operation; this explanation does not assert an execution order for unrelated operands. The operand types and operator implementations determine what `+`, `-`, and `*` do and whether the expression is valid.", src.norm(node), operation, src.norm(*inner), src.norm(other))))
}

pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    let (span, text) = describe(src, node)?;
    Some((span, ReadingContent::Detail { label: "Grouped arithmetic".into(), text, definition: None, borrow: false }))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut items = Vec::new();
    while let Some(part) = pending.pop() {
        if matches!(part.kind(), "macro_invocation" | "token_tree") { continue; }
        if let Some((group, text)) = describe(src, part) {
            items.push(ContractItem { group: "Grouped arithmetic".into(), text, span: src.span(group), type_info: None });
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    items
}
