//! Explain the target-dependent integer types without assuming a target width.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn description(src: Src<'_>, node: Node<'_>) -> Option<String> {
    if node.kind() != "primitive_type" { return None; }
    let (name, signed, range) = match src.text(node) {
        "usize" => ("usize", "unsigned, so it cannot be negative", "0 through 2^N - 1"),
        "isize" => ("isize", "signed, so it can represent negative and positive values", "-2^(N-1) through 2^(N-1) - 1"),
        _ => return None,
    };
    Some(format!("{name} is a pointer-sized integer: its width N is the target platform's pointer width, commonly 32 or 64 bits, and is not chosen from the machine running Nodex. It is {signed}; for N bits its range is {range}. usize is used for indexes and collection lengths; isize is useful for signed pointer-sized differences."))
}

/// Explain types in a symbol's header; a body or nested symbol is handled separately.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let mut pending = vec![node];
    let mut found = Vec::new();
    while let Some(part) = pending.pop() {
        if part != node && matches!(part.kind(), "block" | "declaration_list" | "field_declaration_list" | "ordered_field_declaration_list" | "enum_variant_list" | "token_tree") { continue; }
        if let Some(text) = description(src, part) {
            found.push(ContractItem { group: "Pointer-sized integer".into(), text, span: src.span(part), type_info: None });
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    found
}

/// Attach type details occurring in executable code to its enclosing flow node.
pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<ReadingContent> {
    Some(ReadingContent::Detail { label: "Pointer-sized integer".into(), text: description(src, node)?, definition: None, borrow: false })
}
