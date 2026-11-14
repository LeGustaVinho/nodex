use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Describes simple multi-segment imports on their owning module's contract.
pub(crate) fn explain(src: Src, node: Node) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new() };
    named_children(container).into_iter().filter_map(|item| {
        if item.kind() != "use_declaration" { return None; }
        let path = item.child_by_field_name("argument")?;
        if path.kind() != "scoped_identifier" { return None; }
        let written = src.norm(path);
        let components: Vec<_> = written.split("::").filter(|part| !part.is_empty()).collect();
        if components.len() < 3 { return None; }
        let name = components.last()?;
        Some(ContractItem {
            group: "Import".into(),
            text: format!("{written} identifies an item through its path; :: separates its path segments. This use declaration brings the name {name} into this module's scope, so it can be written without repeating the full path. It imports this item, not every item in the containing module."),
            span: src.span(item),
            type_info: None,
        })
    }).collect()
}
