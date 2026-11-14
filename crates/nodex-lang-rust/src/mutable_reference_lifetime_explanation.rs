//! Explain the written lifetime on an exclusive reference type.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn references<'t>(root: Node<'t>, skip: Option<Node<'t>>) -> Vec<(Node<'t>, Node<'t>)> {
    let mut pending = vec![root];
    let mut found = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == skip || node.kind() == "token_tree" { continue; }
        if node.kind() == "reference_type"
            && children(node).iter().any(|part| part.kind() == "mutable_specifier")
            && let Some(lifetime) = named_children(node).into_iter().find(|part| part.kind() == "lifetime")
        {
            found.push((node, lifetime));
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    found
}

fn text(src: Src<'_>, reference: Node<'_>, lifetime: Node<'_>) -> String {
    let name = src.norm(lifetime);
    let lifetime_text = match name.as_str() {
        "'_" => "The placeholder '_ asks Rust to infer the lifetime from the surrounding context.".to_owned(),
        "'static" => "'static is the program-long lifetime; it constrains how long referenced data must be valid, not how long this particular reference must be kept.".to_owned(),
        _ => format!("{name} is a lifetime name: it expresses a validity relationship checked by the compiler, not a runtime timer. Other references using the same name have a related lifetime requirement; it does not force their values to be created or dropped together."),
    };
    format!("In {}, & means a reference and mut permits changing the borrowed value through it. This is an exclusive borrow: conflicting access is restricted while the borrow is active. The referenced data must remain valid for the lifetime required by this reference. {lifetime_text} A lifetime annotation does not extend the data's actual lifetime or take ownership.", src.norm(reference))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type") { return Vec::new(); }
    references(node, node.child_by_field_name("body")).into_iter().map(|(reference, lifetime)| ContractItem {
        group: "Mutable reference lifetime".into(), text: text(src, reference, lifetime), span: src.span(lifetime), type_info: None,
    }).collect()
}

pub(crate) fn local<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    references(ty, None).into_iter().map(|(reference, lifetime)| (lifetime, ReadingContent::Detail {
        label: "Mutable reference lifetime".into(), text: text(src, reference, lifetime), definition: None, borrow: false,
    })).collect()
}
