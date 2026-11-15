//! Explain the crate-level `#![no_main]` inner attribute.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    // Attach to this file's module symbol, not the next declaration or nested modules.
    if node.kind() != "source_file" { return Vec::new(); }
    named_children(node).into_iter().filter_map(|part| {
        if part.kind() != "inner_attribute_item" || part.has_error() { return None; }
        let attribute = named_children(part).into_iter().find(|child| child.kind() == "attribute")?;
        if attribute.child_by_field_name("arguments").is_some() || attribute.child_by_field_name("value").is_some() { return None; }
        let path = named_children(attribute).into_iter().next()?;
        if src.text(path) != "no_main" { return None; }
        Some(ContractItem {
            group: "No generated main entry point".into(),
            text: "`#![no_main]` is an inner attribute: when placed at the crate root, it applies to the enclosing crate rather than the following item. It tells Rust not to emit its usual main entry-point symbol. It does not itself provide an entry point: an executable may need an entry point supplied by the target, runtime, linker, or other code. It does not by itself disable the standard library (`#![no_std]` is a separate setting). Placement and the final executable's requirements are checked by the compiler and target toolchain.".into(),
            span: src.span(part), type_info: None,
        })
    }).collect()
}
