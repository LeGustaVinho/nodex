//! Explain the Rust string slice type in type annotations.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn is_str(src: Src<'_>, node: Node<'_>) -> bool {
    node.kind() == "primitive_type" && src.text(node) == "str"
}

const TEXT: &str = "Rust's built-in str is a dynamically sized sequence of bytes that must contain valid UTF-8 text. It has no fixed size known at compile time, so it is usually used behind a reference such as &str; that reference describes a borrowed portion of text and carries its length. str is not the same as String, which owns a growable buffer. The meaning of an unqualified name depends on name resolution if a name shadows the built-in type.";

/// Explain occurrences in the declaration's own signature, leaving nested items
/// and bodies to their own symbols or flow readings.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let mut found = Vec::new();
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if current != node && matches!(current.kind(), "block" | "declaration_list" | "field_declaration_list" | "enum_variant_list") { continue; }
        if is_str(src, current) {
            found.push(ContractItem { group: "String slice type".into(), text: TEXT.into(), span: src.span(current), type_info: None });
        } else {
            pending.extend(named_children(current).into_iter().rev());
        }
    }
    found
}

/// Let the flow view show local type annotations when it can attach the span.
pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<ReadingContent> {
    is_str(src, node).then(|| ReadingContent::Detail { label: "String slice type".into(), text: TEXT.into(), definition: None, borrow: false })
}
