//! Explain the equivalence between outer documentation comments and attributes.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if matches!(node.kind(), "source_file" | "attribute_item" | "inner_attribute_item") { return Vec::new(); }
    let mut output = Vec::new();
    let mut previous = node.prev_sibling();
    while let Some(part) = previous {
        previous = part.prev_sibling();
        if part.has_error() { continue; }
        let text = src.text(part);
        if part.kind() == "line_comment" {
            if let Some(content) = text.strip_prefix("///").filter(|_| !text.starts_with("////")) {
                let content = content.trim_end_matches(['\r', '\n']);
                output.push(ContractItem {
                    group: "Outer documentation equivalence".into(),
                    text: format!("`///` introduces an outer documentation comment for the following item. Its text after the three slashes is {:?}; the space after the slashes, if written, is part of that text. Rust treats this as a `#[doc = ...]` attribute with that string value: for example, `/// text` corresponds to `#[doc = \" text\"]`. Rustdoc can render this as item documentation, commonly Markdown; it is not an instruction executed at runtime. This is distinct from `//!`, which documents the enclosing item.", content),
                    span: src.span(part), type_info: None,
                });
            } else if text.starts_with("//!") { break; }
            continue;
        }
        if part.kind() == "block_comment" { if text.starts_with("/*!") { break; } continue; }
        if part.kind() != "attribute_item" { break; }
        let Some(attribute) = named_children(part).into_iter().find(|n| n.kind() == "attribute") else { continue; };
        let Some(value) = attribute.child_by_field_name("value").filter(|n| matches!(n.kind(), "string_literal" | "raw_string_literal")) else { continue; };
        if !named_children(attribute).into_iter().any(|n| n != value && src.text(n) == "doc") || !children(attribute).iter().any(|n| n.kind() == "=") { continue; }
        let written = src.text(value);
        if !(written.starts_with('"') || written.starts_with('r')) { continue; }
        output.push(ContractItem {
            group: "Outer documentation equivalence".into(),
            text: format!("`#[doc = {written}]` is an outer documentation attribute on the following item. A `///` comment supplies the same kind of attribute: `/// text` corresponds to `#[doc = \" text\"]`, including the leading space. The written string literal is interpreted according to Rust string rules. Rustdoc can render the resulting text as item documentation, commonly Markdown; it is not an instruction executed at runtime. This is distinct from `//!`, which documents the enclosing item."),
            span: src.span(part), type_info: None,
        });
    }
    output.reverse();
    output
}
