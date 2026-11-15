//! Explain the crate-root `#![no_builtins]` inner attribute.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    // Only direct inner attributes belong to this source-file module.
    if node.kind() != "source_file" { return Vec::new(); }
    named_children(node).into_iter().filter_map(|part| {
        if part.kind() != "inner_attribute_item" || part.has_error() { return None; }
        let attribute = named_children(part).into_iter().find(|child| child.kind() == "attribute")?;
        if attribute.child_by_field_name("arguments").is_some() || attribute.child_by_field_name("value").is_some() { return None; }
        let path = named_children(attribute).into_iter().next()?;
        if src.text(path) != "no_builtins" { return None; }
        Some(ContractItem {
            group: "Disable selected builtin-call optimizations".into(),
            text: "`#![no_builtins]` is an inner attribute allowed at the crate root. When accepted there, it applies to the enclosing crate and disables certain compiler optimizations involving calls to library functions the compiler would otherwise assume are available. It does not disable all optimizations, remove Rust's built-in types, or prove that no library calls remain. Only the first occurrence has an effect; later occurrences can trigger a lint. The written attribute alone does not establish the actual build configuration or resulting machine code.".into(),
            span: src.span(part), type_info: None,
        })
    }).collect()
}
