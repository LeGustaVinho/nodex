//! Explain `extern crate self as name` without inventing a dependency.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    named_children(container).into_iter().filter_map(|item| {
        if item.kind() != "extern_crate_declaration" || item.has_error() { return None; }
        let name = item.child_by_field_name("name")?;
        let alias = item.child_by_field_name("alias")?;
        if src.text(name) != "self" || src.text(alias) == "_" { return None; }
        let alias = src.norm(alias);
        Some(ContractItem {
            group: "Alias for the current crate".into(),
            text: format!("extern crate self as {alias} binds the name {alias} to the root of the current crate in this scope. The word self refers to the current crate, not an external dependency; as supplies the name that this form requires. Code can use {alias} in paths to items of the current crate, subject to normal scope and visibility rules. This declaration does not itself download a package or create a new crate."),
            span: src.span(item), type_info: None,
        })
    }).collect()
}
