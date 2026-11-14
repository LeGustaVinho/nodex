//! Explain the unit type and its sole value in their respective contexts.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Describe unit types in a symbol's declaration, without walking its body.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let mut pending = vec![node];
    let mut items = Vec::new();
    while let Some(part) = pending.pop() {
        if part.kind() == "unit_type" {
            items.push(ContractItem {
                group: "Unit type".into(),
                text: "() is the unit type. It has exactly one value, also written (). It commonly indicates that no meaningful result is returned; unlike the never type !, a function returning () can finish normally.".into(),
                span: src.span(part), type_info: None,
            });
            continue;
        }
        if part != node && matches!(part.kind(), "block" | "declaration_list") { continue; }
        pending.extend(named_children(part).into_iter().rev());
    }
    items
}

pub(crate) fn value(node: Node<'_>) -> Option<ReadingContent> {
    (node.kind() == "unit_expression").then(|| ReadingContent::Detail {
        label: "Unit value".into(),
        text: "() is the sole value of the unit type, also written (). It carries no information and can be returned normally, for example when a function has no meaningful result.".into(),
        definition: None, borrow: false,
    })
}
