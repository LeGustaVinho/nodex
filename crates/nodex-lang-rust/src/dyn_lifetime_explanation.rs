//! Explain an explicit lifetime bound on a Rust trait object.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn has_dynamic(node: Node<'_>) -> bool {
    node.kind() == "dynamic_type" || (node.kind() == "bounded_type" && named_children(node).into_iter().any(has_dynamic))
}

fn bounds<'t>(root: Node<'t>, skip: Option<Node<'t>>) -> Vec<Node<'t>> {
    let mut todo = vec![root];
    let mut result = Vec::new();
    while let Some(node) = todo.pop() {
        if Some(node) == skip || node.kind() == "token_tree" { continue; }
        if node.kind() == "bounded_type" && has_dynamic(node) {
            result.extend(named_children(node).into_iter().filter(|n| n.kind() == "lifetime"));
        }
        todo.extend(named_children(node).into_iter().rev());
    }
    result
}

fn text(src: Src<'_>, lifetime: Node<'_>) -> String {
    let lt = src.norm(lifetime);
    format!("In a trait object such as dyn Drawable + {lt}, + {lt} is a lifetime bound on the erased concrete type: references stored inside that type must remain valid for at least {lt}. It does not require the particular object value to stay alive until the end of {lt}; it can be dropped earlier. A reference to the trait object has its own borrow lifetime, which is distinct from this bound. The named lifetime is a relationship checked by the compiler, not a runtime timer.")
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type") { return Vec::new(); }
    bounds(node, node.child_by_field_name("body")).into_iter().map(|lifetime| ContractItem {
        group: "Trait object lifetime bound".into(), text: text(src, lifetime), span: src.span(lifetime), type_info: None,
    }).collect()
}

pub(crate) fn local<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    bounds(ty, None).into_iter().map(|lt| (lt, ReadingContent::Detail {
        label: "Trait object lifetime bound".into(), text: text(src, lt), definition: None, borrow: false,
    })).collect()
}
