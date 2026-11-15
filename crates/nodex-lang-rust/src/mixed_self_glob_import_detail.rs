//! Explain a grouped self alias and nested glob import.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => node,
        "mod_item" => match node.child_by_field_name("body") { Some(body) => body, None => return Vec::new() },
        _ => return Vec::new(),
    };
    let mut items = Vec::new();
    for import in named_children(container).into_iter().filter(|part| part.kind() == "use_declaration" && !part.has_error()) {
        let Some(tree) = import.child_by_field_name("argument") else { continue; };
        if tree.kind() != "scoped_use_list" { continue; }
        let (Some(path), Some(list)) = (tree.child_by_field_name("path"), tree.child_by_field_name("list")) else { continue; };
        if src.norm(path) != "crate" { continue; }
        let members: Vec<_> = named_children(list).into_iter().filter(|part| !part.is_extra()).collect();
        if members.len() != 2 { continue; }
        let Some(alias_clause) = members.iter().find(|part| part.kind() == "use_as_clause") else { continue; };
        let (Some(self_path), Some(alias)) = (alias_clause.child_by_field_name("path"), alias_clause.child_by_field_name("alias")) else { continue; };
        if src.text(self_path) != "self" { continue; }
        let Some(glob) = members.iter().find(|part| part.kind() == "use_wildcard") else { continue; };
        let Some(nested) = named_children(*glob).into_iter().next() else { continue; };
        let (prefix, alias, child) = (src.norm(path), src.norm(alias), src.norm(nested));
        items.push(ContractItem {
            group: "Grouped self alias and glob import".into(),
            text: format!("The braces share the prefix `{prefix}` across two imports. `self as {alias}` imports the prefix itself under the local name `{alias}`; `{child}::*` imports eligible visible names from `{prefix}::{child}` into the current scope, rather than importing that module under the name `{child}`. For this example, the corresponding separate forms are `use {prefix} as {alias};` and `use {prefix}::{child}::*;`. Here `crate` as the prefix denotes the current crate's root. Imports introduce names into scope; they do not create the underlying items. Resolution, visibility and conflicts determine which imports are valid and which names are available."),
            span: src.span(list), type_info: None,
        });
    }
    items
}
