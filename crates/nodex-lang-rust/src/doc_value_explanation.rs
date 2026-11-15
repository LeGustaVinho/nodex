//! Explain a direct outer `#[doc = "..."]` documentation attribute.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if matches!(node.kind(), "source_file" | "attribute_item" | "inner_attribute_item") { return Vec::new(); }
    let mut output = Vec::new();
    let mut previous = node.prev_sibling();
    while let Some(part) = previous {
        previous = part.prev_sibling();
        match part.kind() {
            "attribute_item" => {},
            "line_comment" if !src.text(part).starts_with("//!") => continue,
            "block_comment" if !src.text(part).starts_with("/*!") => continue,
            _ => break,
        }
        if part.has_error() { continue; }
        let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute") else { continue; };
        let Some(value) = attribute.child_by_field_name("value") else { continue; };
        if !matches!(value.kind(), "string_literal" | "raw_string_literal") { continue; }
        let written_value = src.text(value);
        if !(written_value.starts_with('"') || written_value.starts_with('r')) { continue; }
        let Some(path) = named_children(attribute).into_iter().find(|child| *child != value) else { continue; };
        if src.text(path) != "doc" || !children(attribute).iter().any(|child| child.kind() == "=") { continue; }
        output.push(ContractItem {
            group: "Documentation value attribute".into(),
            text: format!("`#[doc = ...]` is an outer documentation attribute attached to the following item. Its written value is {} (the literal is interpreted according to its Rust string syntax). Rustdoc normally includes that value in the item's documentation and renders it as Markdown; eligible Rust code examples in documentation can also be run as doctests. The text is not executed as part of the program's normal runtime. Outer `///` documentation comments also supply documentation for the following item.", written_value),
            span: src.span(part), type_info: None,
        });
    }
    output
}
