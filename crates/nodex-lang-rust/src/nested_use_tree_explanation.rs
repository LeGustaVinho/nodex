//! Expand nested grouped imports without assuming name resolution.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn expand(src: Src<'_>, node: Node<'_>, prefix: &str, depth: usize, paths: &mut Vec<String>, nested: &mut bool) -> Option<()> {
    match node.kind() {
        "scoped_use_list" => {
            let path = src.norm(node.child_by_field_name("path")?);
            let prefix = if prefix.is_empty() { path } else { format!("{prefix}::{path}") };
            if depth > 0 { *nested = true; }
            let list = node.child_by_field_name("list")?;
            for child in named_children(list).into_iter().filter(|child| !child.is_extra()) {
                expand(src, child, &prefix, depth + 1, paths, nested)?;
            }
            Some(())
        }
        "identifier" | "scoped_identifier" | "scoped_type_identifier" => {
            let path = src.norm(node);
            if path == "self" { return None; }
            paths.push(if prefix.is_empty() { path } else { format!("{prefix}::{path}") });
            Some(())
        }
        _ => None,
    }
}

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    named_children(container).into_iter().filter(|item| item.kind() == "use_declaration" && !item.has_error()).filter_map(|item| {
        let mut paths = Vec::new();
        let mut nested = false;
        expand(src, item.child_by_field_name("argument")?, "", 0, &mut paths, &mut nested)?;
        if !nested || paths.is_empty() { return None; }
        Some(ContractItem {
            group: "Nested grouped imports".into(),
            text: format!("The braces group paths that share prefixes; each nested group adds its prefix to the entries inside it. Written as separate imports, these paths are: {}. `use` brings the final names into this module's scope; it refers to existing items rather than defining them. The compiler still resolves the paths and checks visibility, conflicts, and whether the imports are valid. This description does not prove any item exists.", paths.iter().map(|path| format!("`use {path};`")).collect::<Vec<_>>().join(", ")),
            span: src.span(item), type_info: None,
        })
    }).collect()
}
