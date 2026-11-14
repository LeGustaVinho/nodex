//! Explain named lifetime annotations on shared references.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn occurrences<'t>(root: Node<'t>, skip: Option<Node<'t>>) -> Vec<Node<'t>> {
    let mut todo = vec![root];
    let mut result = Vec::new();
    while let Some(node) = todo.pop() {
        if Some(node) == skip || node.kind() == "token_tree" { continue; }
        if node.kind() == "reference_type"
            && !children(node).iter().any(|child| child.kind() == "mutable_specifier")
        {
            if let Some(lifetime) = named_children(node).into_iter().find(|child| child.kind() == "lifetime") {
                result.push(lifetime);
            }
        }
        todo.extend(named_children(node).into_iter().rev());
    }
    result
}

fn description(src: Src<'_>, lifetime: Node<'_>) -> Option<String> {
    let lt = src.norm(lifetime);
    if lt == "'_" || lt == "'static" { return None; }
    Some(format!("{lt} names a lifetime in this shared reference type. It describes a region for which the reference may be used, and the compiler checks that the borrowed data is valid throughout that region. Reusing {lt} in a function signature relates the references or bounds that name it; the actual valid region must satisfy all those constraints. The annotation does not extend the data's lifetime or keep it alive, and is not a runtime timer."))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type") { return Vec::new(); }
    occurrences(node, node.child_by_field_name("body")).into_iter().filter_map(|lt| Some(ContractItem {
        group: "Named reference lifetime".into(), text: description(src, lt)?, span: src.span(lt), type_info: None,
    })).collect()
}

pub(crate) fn local<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    occurrences(ty, None).into_iter().filter_map(|lt| Some((lt, ReadingContent::Detail {
        label: "Named reference lifetime".into(), text: description(src, lt)?, definition: None, borrow: false,
    }))).collect()
}
