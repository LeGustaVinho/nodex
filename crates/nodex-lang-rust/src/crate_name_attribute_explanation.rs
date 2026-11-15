//! Explain the crate-level `#![crate_name = "..."]` attribute.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    // An isolated source file need not be a crate root. Attach only to the file's
    // own module, and state the placement requirement rather than assuming it.
    if node.kind() != "source_file" { return Vec::new(); }
    named_children(node).into_iter().filter_map(|item| {
        if item.kind() != "inner_attribute_item" || item.has_error() { return None; }
        let attribute = named_children(item).into_iter().find(|child| child.kind() == "attribute")?;
        let value = attribute.child_by_field_name("value")?;
        let name = named_children(attribute).into_iter().find(|child| *child != value)?;
        if src.norm(name) != "crate_name" || !matches!(value.kind(), "string_literal" | "raw_string_literal") || value.has_error() { return None; }
        Some(ContractItem {
            group: "Crate name attribute".into(),
            text: format!("`#![crate_name = ...]` is an inner attribute: when placed at the crate root, it specifies the name of the current crate (a Rust compilation unit), using the written string {}. It is not a runtime assignment or the name of only the next item. The compiler requires a nonempty name containing only Unicode alphanumeric characters or underscores. If `--crate-name` is also provided, the names must agree; Cargo normally supplies this option from its target configuration. This isolated source view does not by itself establish that the file is the actual crate root or reveal the build's effective name and flags.", src.text(value)),
            span: src.span(item), type_info: None,
        })
    }).collect()
}
