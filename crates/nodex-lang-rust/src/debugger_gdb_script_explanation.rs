//! Explain a GDB debugger visualizer named by an inner attribute.
//! Rust Reference: https://doc.rust-lang.org/reference/attributes/debugger.html
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    named_children(container).into_iter().filter_map(|item| {
        if item.kind() != "inner_attribute_item" || item.has_error() { return None; }
        let attr = named_children(item).into_iter().find(|part| part.kind() == "attribute")?;
        let path = named_children(attr).into_iter().next()?;
        if src.norm(path) != "debugger_visualizer" { return None; }
        let args = attr.child_by_field_name("arguments")?;
        let tokens: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
        let [left, key, eq, value, right] = tokens.as_slice() else { return None; };
        if left.kind() != "(" || src.text(*key) != "gdb_script_file" || eq.kind() != "=" || value.kind() != "string_literal" || right.kind() != ")" { return None; }
        Some(ContractItem {
            group: "GDB debugger visualizer".into(),
            text: format!("`#![debugger_visualizer(gdb_script_file = {})]` is an inner attribute for this crate or module. It tells Rust to embed a GDB Python visualizer file into debug information; the written path {} is relative to the source file. Such a script can customize how GDB displays values. This does not execute the script as part of ordinary program execution, and GDB may require explicit auto-load permission before loading embedded pretty printers. The attribute alone does not prove that the file exists, the build supports embedding it, or GDB will load it.", src.text(*value), src.text(*value)),
            span: src.span(item), type_info: None,
        })
    }).collect()
}
