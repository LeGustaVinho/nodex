//! Explain `#![windows_subsystem = "windows"]` without guessing the build target.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    // The direct inner attribute belongs to the source-file module, not a child item.
    if node.kind() != "source_file" { return Vec::new(); }
    named_children(node).into_iter().filter_map(|part| {
        if part.kind() != "inner_attribute_item" || part.has_error() { return None; }
        let attribute = named_children(part).into_iter().find(|child| child.kind() == "attribute")?;
        let value = attribute.child_by_field_name("value")?;
        if value.kind() != "string_literal" || src.text(value) != "\"windows\"" { return None; }
        let path = named_children(attribute).into_iter().find(|child| *child != value)?;
        if src.text(path) != "windows_subsystem" || !children(attribute).iter().any(|child| child.kind() == "=") { return None; }
        Some(ContractItem {
            group: "Windows GUI subsystem".into(),
            text: "`#![windows_subsystem = \"windows\"]` is an inner attribute allowed at the crate root. When active for a Windows binary, it selects the `windows` subsystem during linking; that process runs detached from an existing console. This setting is commonly used for GUI applications, but it does not create a GUI by itself. The attribute is ignored for non-Windows targets and non-binary crate types; only the first setting takes effect. The written source alone does not establish the target, crate type, or final link configuration.".into(),
            span: src.span(part), type_info: None,
        })
    }).collect()
}
