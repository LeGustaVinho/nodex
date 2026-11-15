//! Explain outer name-value attributes with a written string value.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "enum_variant" | "union_item" | "trait_item" | "type_item" | "associated_type" | "const_item" | "static_item" | "field_declaration" | "mod_item" | "macro_definition" | "impl_item") { return Vec::new(); }
    let mut previous = node.prev_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" => {
                if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute") {
                    if let Some(value) = attribute.child_by_field_name("value") {
                        let path = named_children(attribute).into_iter().find(|child| *child != value);
                        if let Some(path) = path && matches!(value.kind(), "string_literal" | "raw_string_literal") && !value.has_error() {
                            result.push(ContractItem {
                                group: "Name-value attribute".into(),
                                text: format!("This outer attribute attaches metadata to the following item. Its name is {} and its written value is {}. The `=` separates the attribute name and value; it is not an assignment to a runtime variable. The meaning, accepted value and effect depend on the particular attribute and compiler or tool that handles it. The spelling alone does not establish the attribute's resolution or prove what transformation or effect it has.", src.norm(path), src.text(value)),
                                span: src.span(part), type_info: None,
                            });
                        }
                    }
                }
            }
            "line_comment" if src.text(part).starts_with("//!") => break,
            "block_comment" if src.text(part).starts_with("/*!") => break,
            "line_comment" | "block_comment" => {},
            _ => break,
        }
        previous = part.prev_sibling();
    }
    result.reverse();
    result
}
