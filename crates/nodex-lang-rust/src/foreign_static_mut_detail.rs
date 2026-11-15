//! Explain mutable foreign static declarations on their owning module.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => node,
        "mod_item" => match node.child_by_field_name("body") { Some(body) => body, None => return Vec::new() },
        _ => return Vec::new(),
    };
    let mut result = Vec::new();
    for external in named_children(container).into_iter().filter(|part| part.kind() == "foreign_mod_item") {
        if !children(external).iter().any(|part| part.kind() == "unsafe") { continue; }
        let Some(modifier) = named_children(external).into_iter().find(|part| part.kind() == "extern_modifier") else { continue; };
        if !children(modifier).iter().any(|part| part.kind() == "string_literal" && src.text(*part) == "\"C\"") { continue; }
        let Some(body) = external.child_by_field_name("body") else { continue; };
        for item in named_children(body).into_iter().filter(|part| part.kind() == "static_item" && !part.has_error()) {
            let Some(mutability) = named_children(item).into_iter().find(|part| part.kind() == "mutable_specifier") else { continue; };
            let (Some(name), Some(ty)) = (item.child_by_field_name("name"), item.child_by_field_name("type")) else { continue; };
            result.push(ContractItem {
                group: "Mutable foreign static".into(),
                text: format!("`static mut {}: {}` declares mutable static storage supplied by linked code; it does not allocate or initialize the variable here. Rust treats reads or writes of a mutable static as unsafe operations, even if a foreign item is declared safe. The programmer must ensure the foreign definition has the declared type and a valid value and prevent data races and invalid aliasing when Rust or foreign code accesses the storage. `mut` permits mutation; it does not provide synchronization. Creating references to a mutable static has additional aliasing requirements and can trigger the `static_mut_refs` lint. The extern block and C ABI do not prove the external symbol exists, links, or is safe to access.", src.norm(name), src.norm(ty)),
                span: src.span(mutability), type_info: None,
            });
        }
    }
    result
}
