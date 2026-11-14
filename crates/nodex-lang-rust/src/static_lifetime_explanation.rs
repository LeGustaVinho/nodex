//! Explain the special `'static` lifetime without promising that a value lives forever.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn written_static(src: Src<'_>, node: Node<'_>) -> bool {
    node.kind() == "lifetime" && src.norm(node) == "'static"
}

fn explanation() -> &'static str {
    "'static denotes a lifetime that spans the whole program. For a reference such as &'static str, the referenced data must remain valid for that lifetime; string literals are one example. A bound such as T: 'static instead requires that T contain no borrowed data with a shorter lifetime. It does not mean a particular value of T must live forever: owned values can still be dropped earlier. These are compile-time validity constraints, not a timer that keeps a value alive."
}

fn lifetimes<'t>(src: Src<'_>, root: Node<'t>, skip: Option<Node<'t>>) -> Vec<Node<'t>> {
    let mut pending = vec![root];
    let mut found = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == skip || matches!(node.kind(), "token_tree" | "block" | "declaration_list" | "field_declaration_list" | "ordered_field_declaration_list" | "enum_variant_list") { continue; }
        if written_static(src, node) { found.push(node); }
        pending.extend(named_children(node).into_iter().rev());
    }
    found
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type" | "struct_item" | "trait_item") { return Vec::new(); }
    lifetimes(src, node, node.child_by_field_name("body")).into_iter().map(|lt| ContractItem {
        group: "Static lifetime".into(), text: explanation().into(), span: src.span(lt), type_info: None,
    }).collect()
}

pub(crate) fn local(src: Src<'_>, node: Node<'_>) -> Vec<(Span, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    lifetimes(src, ty, None).into_iter().map(|lt| (src.index.span(src.file, lt.start_byte() + 1, lt.end_byte()), ReadingContent::Detail {
        label: "Static lifetime".into(), text: explanation().into(), definition: None, borrow: false,
    })).collect()
}
