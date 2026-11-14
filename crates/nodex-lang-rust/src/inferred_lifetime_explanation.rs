//! Explain the anonymous lifetime placeholder written as `'_`.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

const TEXT: &str = "'_ is an anonymous lifetime placeholder. The compiler infers the lifetime required at this position from the surrounding context and Rust's lifetime rules. It is not a named lifetime shared automatically with other occurrences, and it is not a runtime value or a request to keep data alive forever. The inferred lifetime still has to satisfy borrowing and validity requirements.";

fn placeholders<'t>(src: Src<'_>, root: Node<'t>, skip: Option<Node<'t>>) -> Vec<Node<'t>> {
    let mut stack = vec![root];
    let mut found = Vec::new();
    while let Some(node) = stack.pop() {
        if Some(node) == skip || node.kind() == "token_tree" { continue; }
        if node.kind() == "lifetime" && src.norm(node) == "'_" {
            found.push(node);
            continue;
        }
        stack.extend(named_children(node).into_iter().rev());
    }
    found
}

/// Explain only the declaration's own signature; body items get their own readings.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type" | "struct_item" | "enum_item" | "trait_item") { return Vec::new(); }
    placeholders(src, node, node.child_by_field_name("body")).into_iter().map(|lifetime| ContractItem {
        group: "Inferred lifetime".into(), text: TEXT.into(), span: src.span(lifetime), type_info: None,
    }).collect()
}

/// Explain placeholders explicitly written in a local type annotation.
pub(crate) fn local(src: Src<'_>, node: Node<'_>) -> Vec<(Span, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    placeholders(src, ty, None).into_iter().map(|lifetime| (src.index.span(src.file, lifetime.start_byte() + 1, lifetime.end_byte()), ReadingContent::Detail {
        label: "Inferred lifetime".into(), text: TEXT.into(), definition: None, borrow: false,
    })).collect()
}
