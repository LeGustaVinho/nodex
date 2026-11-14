//! Explanations for `self::name` paths in imports and expressions.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn is_self_path(src: Src<'_>, node: Node<'_>) -> bool {
    node.kind() == "scoped_identifier"
        && node.child_by_field_name("path").is_some_and(|path| src.norm(path) == "self")
        && node.child_by_field_name("name").is_some()
}

fn text(src: Src<'_>, node: Node<'_>) -> String {
    let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
    format!("self refers to the current module here. The :: separator follows a path, so self::{name} refers to {name} inside that module. This self is a module path, not the receiver of a method.")
}

/// Invoke on expression path nodes from the flow reader.
pub(crate) fn expression(src: Src<'_>, node: Node<'_>) -> Option<ReadingContent> {
    if !is_self_path(src, node) { return None; }
    Some(ReadingContent::Detail {
        label: "Name in the current module".into(),
        text: text(src, node),
        definition: None,
        borrow: false,
    })
}

/// Attach direct `use self::name` declarations to their containing module.
pub(crate) fn imports(src: Src<'_>, module: Node<'_>) -> Vec<ContractItem> {
    let container = match module.kind() {
        "source_file" => Some(module),
        "mod_item" => module.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new() };
    named_children(container).into_iter().filter_map(|declaration| {
        if declaration.kind() != "use_declaration" { return None; }
        let path = declaration.child_by_field_name("argument")?;
        if !is_self_path(src, path) { return None; }
        Some(ContractItem {
            group: "Current module path".into(),
            text: format!("{} The use declaration brings that name into the current scope, subject to visibility.", text(src, path)),
            span: src.span(path),
            type_info: None,
        })
    }).collect()
}
