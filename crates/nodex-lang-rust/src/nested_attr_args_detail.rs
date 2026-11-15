//! Explain mixed path and named string input in outer attributes.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if matches!(node.kind(), "source_file" | "attribute_item" | "inner_attribute_item") { return Vec::new(); }
    let mut prev = node.prev_sibling();
    let mut output = Vec::new();
    while let Some(part) = prev {
        prev = part.prev_sibling();
        match part.kind() {
            "attribute_item" => {},
            "line_comment" if !src.text(part).starts_with("//!") => continue,
            "block_comment" if !src.text(part).starts_with("/*!") => continue,
            _ => break,
        }
        if part.has_error() { continue; }
        let Some(attr) = named_children(part).into_iter().find(|child| child.kind() == "attribute") else { continue; };
        let Some(args) = attr.child_by_field_name("arguments") else { continue; };
        let Some(path) = named_children(attr).into_iter().find(|child| *child != args) else { continue; };
        let tokens: Vec<_> = children(args).into_iter().filter(|child| !child.is_extra()).collect();
        if tokens.len() < 3 || tokens[0].kind() != "(" || tokens.last().unwrap().kind() != ")" { continue; }
        let segments: Vec<_> = tokens[1..tokens.len()-1].split(|child| child.kind() == ",").filter(|segment| !segment.is_empty()).collect();
        if segments.len() != 2 { continue; }
        let path_parts = segments[0];
        let path_ok = path_parts.len() >= 3 && path_parts.len() % 2 == 1 && path_parts.iter().enumerate().all(|(index, part)| if index % 2 == 0 { part.kind() == "identifier" } else { part.kind() == "::" });
        let [key, equal, value] = segments[1] else { continue; };
        if !path_ok || key.kind() != "identifier" || equal.kind() != "=" || !matches!(value.kind(), "string_literal" | "raw_string_literal") { continue; }
        let written_path = &src.text[path_parts[0].start_byte()..path_parts.last().unwrap().end_byte()];
        output.push(ContractItem {
            group: "Attribute with path and named input".into(),
            text: format!("`#[...]` attaches an outer attribute to the following item. `{}` is the attribute's written name; the parentheses contain its input tokens. `{}` is a path written with `::` separators, and `{} = {}` pairs a name with a string literal. The `=` here is part of the attribute input, not a runtime assignment. An attribute macro can interpret its input tokens itself; acceptance and meaning depend on the resolved attribute and compiler or tool, so this syntax alone does not establish the effect or resolution.", src.norm(path), written_path, src.text(*key), src.text(*value)),
            span: src.span(part), type_info: None,
        });
    }
    output
}
