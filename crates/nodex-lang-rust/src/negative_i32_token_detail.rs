//! Explain unary minus applied to a directly suffixed i32 integer literal.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn describe<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, String)> {
    if node.kind() != "unary_expression" || node.has_error() { return None; }
    let minus = children(node).into_iter().find(|part| part.kind() == "-")?;
    let operands: Vec<_> = named_children(node).into_iter().filter(|part| !part.is_extra()).collect();
    let [literal] = operands.as_slice() else { return None; };
    if literal.kind() != "integer_literal" || !src.text(*literal).ends_with("i32") { return None; }
    let spelling = src.text(*literal);
    Some((minus, format!("`{}` applies the unary `-` operator to the integer literal token `{spelling}`; the minus sign is a separate operator, not part of that token. The `i32` suffix specifies a signed 32-bit integer, and for an ordinary in-range positive literal the operator produces its negation (for example, `-123i32` has value -123). The minimum i32 value has a special literal-negation boundary case, so do not assume the positive magnitude is representable as a standalone i32. Rust checks the full expression and overflow rules.", src.norm(node))))
}

pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    let (site, text) = describe(src, node)?;
    Some((site, ReadingContent::Detail { label: "Negative i32 literal expression".into(), text, definition: None, borrow: false }))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut items = Vec::new();
    while let Some(part) = pending.pop() {
        if matches!(part.kind(), "macro_invocation" | "token_tree") { continue; }
        if let Some((site, text)) = describe(src, part) { items.push(ContractItem { group: "Negative i32 literal expression".into(), text, span: src.span(site), type_info: None }); }
        pending.extend(named_children(part).into_iter().rev());
    }
    items
}
