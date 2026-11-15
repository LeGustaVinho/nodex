//! Explain explicit aliases on external crate declarations.
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
    named_children(container).into_iter().filter_map(|declaration| {
        if declaration.kind() != "extern_crate_declaration" || declaration.has_error() { return None; }
        let name = declaration.child_by_field_name("name")?;
        let alias = declaration.child_by_field_name("alias")?;
        let (original, local) = (src.norm(name), src.norm(alias));
        if original == "self" || local == "_" { return None; }
        Some(ContractItem {
            group: "External crate alias".into(),
            text: format!("`extern crate {original} as {local};` explicitly brings the crate named {original} into scope under the local name {local}. In the relevant scope, code can use {local} to refer to that crate; `as` changes the name introduced here, not the underlying crate. A crate is a Rust compilation unit, and the build must make the requested crate available. This older syntax is often unnecessary for ordinary dependencies in Rust 2018 and later, but the source alone does not prove how the dependency is configured or resolved."),
            span: src.span(declaration), type_info: None,
        })
    }).collect()
}
