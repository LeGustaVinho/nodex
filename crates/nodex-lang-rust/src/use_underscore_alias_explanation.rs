//! Describe anonymous imports without assuming path resolution.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new() };
    named_children(container).into_iter().filter_map(|item| {
        if item.kind() != "use_declaration" || item.has_error() { return None; }
        let argument = item.child_by_field_name("argument")?;
        if argument.kind() != "use_as_clause" { return None; }
        let path = argument.child_by_field_name("path")?;
        let alias = argument.child_by_field_name("alias")?;
        if src.norm(alias) != "_" { return None; }
        Some(ContractItem {
            group: "Anonymous import".into(),
            text: format!("Import {} with the anonymous alias `_`: it introduces no usable local name for the imported item. If the path resolves to a trait, its methods can participate in method lookup without introducing the trait's name, subject to Rust's scope and trait rules. The `::` symbols separate path segments; this syntax alone does not establish what the path resolves to or whether the import is valid.", src.norm(path)),
            span: src.span(item), type_info: None,
        })
    }).collect()
}
