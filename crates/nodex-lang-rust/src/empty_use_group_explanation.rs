//! Explain a path followed by an empty import group on its owning module.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, module: Node<'_>) -> Vec<ContractItem> {
    let container = match module.kind() {
        "source_file" => module,
        "mod_item" => match module.child_by_field_name("body") { Some(body) => body, None => return Vec::new() },
        _ => return Vec::new(),
    };
    named_children(container).into_iter().filter_map(|item| {
        if item.kind() != "use_declaration" || item.has_error() { return None; }
        let argument = item.child_by_field_name("argument")?;
        if argument.kind() != "scoped_use_list" { return None; }
        let prefix = argument.child_by_field_name("path")?;
        let list = argument.child_by_field_name("list")?;
        if named_children(list).into_iter().any(|part| !part.is_extra()) { return None; }
        let path = src.norm(prefix);
        Some(ContractItem {
            group: "Empty import group".into(),
            text: format!("`use {path}::{{}};` has an empty list between the braces, so it imports no names and creates no binding for {path}. Rust still checks that the preceding path {path} resolves and is accessible; an unresolved or inaccessible path can cause a compilation error. This source alone does not establish whether the path resolves. A nonempty list such as `{{name}}` would import the listed item instead."),
            span: src.span(item), type_info: None,
        })
    }).collect()
}
