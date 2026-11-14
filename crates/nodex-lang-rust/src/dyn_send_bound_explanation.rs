//! Explain a Send bound on a dynamically dispatched trait object.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn occurrences<'t>(src: Src<'_>, root: Node<'t>) -> Vec<Node<'t>> {
    let mut result = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.kind() == "bounded_type" {
            let parts = named_children(node);
            if parts.iter().any(|part| part.kind() == "dynamic_type") {
                for bound in &parts {
                    let spelling = src.norm(*bound);
                    if spelling == "Send" || spelling == "std::marker::Send" || spelling == "core::marker::Send" {
                        result.push(*bound);
                    }
                }
            }
        }
        pending.extend(named_children(node));
    }
    result.sort_by_key(|node| node.start_byte());
    result
}

fn text(src: Src<'_>, bound: Node<'_>) -> String {
    format!("The + adds another requirement to the trait object: {} must also be satisfied by the concrete value hidden behind dyn. dyn allows calls through the named trait using runtime dispatch; it does not by itself own the value. If {} resolves to Rust's standard Send auto trait, that value may be transferred between threads subject to Rust's other rules. The spelling alone does not prove which trait the name resolves to. This bound does not itself make shared access thread-safe; Sync is a separate requirement.", src.norm(bound), src.norm(bound))
}

fn header_types(node: Node<'_>) -> Vec<Node<'_>> {
    match node.kind() {
        "function_item" | "function_signature_item" => {
            let mut types = node.child_by_field_name("parameters").map(named_children).unwrap_or_default().into_iter().filter_map(|p| p.child_by_field_name("type")).collect::<Vec<_>>();
            types.extend(node.child_by_field_name("return_type"));
            types
        }
        "field_declaration" | "const_item" | "static_item" | "type_item" | "associated_type" => node.child_by_field_name("type").into_iter().collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    header_types(node).into_iter().flat_map(|ty| occurrences(src, ty)).map(|bound| ContractItem {
        group: "Trait object Send bound".into(), text: text(src, bound), span: src.span(bound), type_info: None,
    }).collect()
}

pub(crate) fn local<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    node.child_by_field_name("type").into_iter().flat_map(|ty| occurrences(src, ty)).map(|bound| {
        (bound, ReadingContent::Detail { label: "Trait object Send bound".into(), text: text(src, bound), definition: None, borrow: false })
    }).collect()
}
