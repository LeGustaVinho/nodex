//! Explain a parenthesized outer attribute containing `key = "value"` tokens.
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
        let Some(arguments) = attribute.child_by_field_name("arguments") else { continue; };
        let Some(path) = named_children(attribute).into_iter().find(|child| *child != arguments) else { continue; };
        let tokens: Vec<_> = children(arguments).into_iter().filter(|child| !child.is_extra()).collect();
        if tokens.first().is_none_or(|part| part.kind() != "(") || tokens.last().is_none_or(|part| part.kind() != ")") { continue; }
        let mut i = 1;
        let mut names = Vec::new();
        let mut valid = true;
        while i + 1 < tokens.len() {
            if i + 2 >= tokens.len() - 1 || tokens[i].kind() != "identifier" || tokens[i+1].kind() != "=" || !matches!(tokens[i+2].kind(), "string_literal" | "raw_string_literal") { valid = false; break; }
            names.push(format!("{} = {}", src.text(tokens[i]), src.text(tokens[i+2])));
            i += 3;
            if i == tokens.len() - 1 { break; }
            if tokens[i].kind() != "," { valid = false; break; }
            i += 1;
        }
        if !valid || names.is_empty() || i != tokens.len() - 1 { continue; }
        output.push(ContractItem {
            group: "Attribute with named input".into(),
            text: format!("`#[...]` is an outer attribute attached to the following item. {} is its written path; the parentheses contain the exact input tokens {}. Within that input, {} uses a name, `=`, and a string literal. The meaning and whether this is accepted depend on the resolved attribute: an attribute macro can interpret the tokens itself, while other attributes may use structured metadata. This spelling alone does not establish which attribute is resolved or imply a runtime operation.", src.text(path), src.text(arguments), names.join(", ")),
            span: src.span(part), type_info: None,
        });
    }
    output
}
