use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn crate_rooted(node: Node<'_>) -> bool {
    if !matches!(node.kind(), "scoped_identifier" | "scoped_type_identifier") { return false; }
    let Some(path) = node.child_by_field_name("path") else { return false; };
    path.kind() == "crate" || crate_rooted(path)
}

fn description(src: Src, node: Node<'_>) -> String {
    format!("{} starts at the root module of the current crate (the current Rust compilation unit). The parts after :: select an item through that module's path. This does not mean a filesystem path; normal visibility rules still apply.", src.norm(node))
}

pub(crate) fn expression(src: Src, node: Node<'_>) -> Option<ReadingContent> {
    if !crate_rooted(node) || node.parent().is_some_and(crate_rooted) { return None; }
    Some(ReadingContent::Detail {
        label: "Path from this crate's root".into(),
        text: description(src, node),
        definition: None,
        borrow: false,
    })
}

pub(crate) fn imports(src: Src, module: Node<'_>) -> Vec<ContractItem> {
    let container = match module.kind() {
        "source_file" => module,
        "mod_item" => match module.child_by_field_name("body") { Some(body) => body, None => return Vec::new() },
        _ => return Vec::new(),
    };
    named_children(container).into_iter().filter_map(|item| {
        if item.kind() != "use_declaration" { return None; }
        let path = item.child_by_field_name("argument")?;
        if !crate_rooted(path) { return None; }
        Some(ContractItem {
            group: "Import from this crate".into(),
            text: format!("{} The use declaration brings the selected name into this module's scope.", description(src, path)),
            span: src.span(item),
            type_info: None,
        })
    }).collect()
}
