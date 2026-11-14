//! Explain fully qualified associated-type projections without assuming name resolution.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn projection<'t>(node: Node<'t>) -> Option<(Node<'t>, Node<'t>, Node<'t>, Node<'t>)> {
    if node.kind() != "scoped_type_identifier" { return None; }
    let path = node.child_by_field_name("path")?;
    if path.kind() != "bracketed_type" { return None; }
    let qualified = named_children(path).into_iter().find(|n| n.kind() == "qualified_type")?;
    let ty = qualified.child_by_field_name("type")?;
    let trait_name = qualified.child_by_field_name("alias")?;
    let item = node.child_by_field_name("name")?;
    let as_token = children(qualified).into_iter().find(|n| n.kind() == "as")?;
    Some((ty, trait_name, item, as_token))
}

fn description(src: Src<'_>, node: Node<'_>) -> Option<String> {
    let (ty, trait_name, item, _) = projection(node)?;
    Some(format!("In {}, select the associated type {} from the implementation of trait {} for type {}. The syntax <Type as Trait>::Item explicitly identifies both the type and the trait, which can disambiguate traits that use the same associated name. It refers to a type, not a value; the compiler must still resolve the names and verify that the implementation exists.", src.norm(node), src.norm(item), src.norm(trait_name), src.norm(ty)))
}

fn occurrences<'t>(root: Node<'t>, skip: Option<Node<'t>>) -> Vec<Node<'t>> {
    let mut nodes = vec![root]; let mut found = Vec::new();
    while let Some(node) = nodes.pop() {
        if Some(node) == skip || node.kind() == "token_tree" { continue; }
        if projection(node).is_some() { found.push(node); }
        nodes.extend(named_children(node).into_iter().rev());
    }
    found
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "associated_type" | "const_item" | "static_item" | "trait_item" | "struct_item" | "enum_item") { return Vec::new(); }
    occurrences(node, node.child_by_field_name("body")).into_iter().filter_map(|site| Some(ContractItem {
        group: "Qualified associated type".into(), text: description(src, site)?, span: src.span(site), type_info: None,
    })).collect()
}

pub(crate) fn local<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    occurrences(ty, None).into_iter().filter_map(|site| {
        let (_, _, _, as_token) = projection(site)?;
        Some((as_token, ReadingContent::Detail { label: "Qualified associated type".into(), text: description(src, site)?, definition: None, borrow: false }))
    }).collect()
}
