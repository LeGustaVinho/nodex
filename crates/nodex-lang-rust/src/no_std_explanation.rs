//! Explain the crate-level `no_std` inner attribute.
//! Rust Reference: https://doc.rust-lang.org/reference/names/preludes.html#the-no_std-attribute
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "source_file" { return Vec::new(); }
    named_children(node).into_iter().filter_map(|item| {
        if item.kind() != "inner_attribute_item" || item.has_error() { return None; }
        let compact: String = src.text(item).chars().filter(|ch| !ch.is_whitespace()).collect();
        if compact != "#![no_std]" { return None; }
        Some(ContractItem {
            group: "Crate without automatic std".into(),
            text: "`#![no_std]` is an inner crate attribute intended for the crate root. It prevents automatic linking of Rust's `std` library and uses the `core` prelude instead of the `std` prelude; foundational types such as Option and Result remain available from core. The `alloc` crate can be brought in explicitly for allocation-backed types when allocator support is available. This does not forbid `std`: the crate or a dependency can explicitly link it. A no_std binary may need its own panic handler and runtime entry setup; this attribute alone does not provide them. The file's role as crate root and the build configuration still need to be established separately.".into(),
            span: src.span(item), type_info: None,
        })
    }).collect()
}
