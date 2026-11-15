//! Explain the equivalence of inner line documentation and direct doc attributes.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" | "function_item" | "trait_item" | "impl_item" | "foreign_mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    let mut output = Vec::new();
    for part in children(container) {
        let kind = part.kind();
        if kind == "{" { continue; }
        if kind == "inner_attribute_item" {
            if part.has_error() { continue; }
            let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute") else { continue; };
            let Some(value) = attribute.child_by_field_name("value") else { continue; };
            if !matches!(value.kind(), "string_literal" | "raw_string_literal") || value.has_error() { continue; }
            let Some(name) = named_children(attribute).into_iter().find(|child| *child != value) else { continue; };
            if src.text(name) != "doc" || !children(attribute).iter().any(|child| child.kind() == "=") { continue; }
            output.push(ContractItem {
                group: "Inner documentation equivalence".into(),
                text: format!("`#![doc = ...]` adds documentation to the enclosing crate, module or item, not the following item. Its written string is {}; Rust interprets escapes and raw strings according to their literal syntax. An inner line comment `//! text` supplies an equivalent inner `doc` attribute whose text is the characters after `//!`, including the leading space when present. The text can appear in generated documentation; neither form is executed as a runtime instruction. Exact contents matter, and this source alone does not establish that documentation was generated.", src.text(value)),
                span: src.span(part), type_info: None,
            });
            continue;
        }
        if kind == "line_comment" && src.text(part).starts_with("//!") {
            let content = src.text(part)[3..].trim_end_matches(['\r', '\n']);
            output.push(ContractItem {
                group: "Inner documentation equivalence".into(),
                text: format!("This inner documentation comment supplies text {:?} to the enclosing crate, module or item, not the following item. `//! text` is equivalent to an inner `#![doc = \" text\"]` attribute: the characters after `//!` are the document text, including the leading space. A Rust string attribute uses its own escape and raw-string rules, so matching contents matters. Documentation tools can include this text; it is not a runtime instruction, and this source alone does not establish that documentation was generated.", content),
                span: src.span(part), type_info: None,
            });
            continue;
        }
        if kind == "line_comment" && !src.text(part).starts_with("///") { continue; }
        if kind == "block_comment" && !src.text(part).starts_with("/**") { continue; }
        break;
    }
    output
}
