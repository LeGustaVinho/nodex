//! Explain `#[must_use = "reason"]` while preserving its written message.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let target = match node.kind() {
        "function_item" | "function_signature_item" => "When a call to this function or method is used as an expression statement and its result is ignored,",
        "struct_item" | "enum_item" | "union_item" => "When an expression statement produces a value of this type and ignores it,",
        "trait_item" => "When an ignored function call returns an `impl Trait` or `dyn Trait` whose bounds include this trait,",
        _ => return Vec::new(),
    };
    let mut previous = node.prev_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" if !part.has_error() => {
                if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute")
                    && let Some(path) = named_children(attribute).into_iter().next()
                    && src.text(path) == "must_use"
                    && let Some(value) = attribute.child_by_field_name("value")
                    && matches!(value.kind(), "string_literal" | "raw_string_literal")
                    && (src.text(value).starts_with('"') || src.text(value).starts_with('r'))
                {
                    result.push(ContractItem {
                        group: "Must-use reason".into(),
                        text: format!("`#[must_use = ...]` marks this item as one whose result is important to consider. {target} Rust's `unused_must_use` lint may report it. The reason written in the attribute is {}; the compiler may include that message in the diagnostic. The lint normally warns, but lint settings can allow it or make it an error. This attribute does not execute the item or force a caller at runtime to inspect the result; the compiler still checks whether this attribute and its message are valid here.", src.text(value)),
                        span: src.span(part), type_info: None,
                    });
                }
            }
            "line_comment" if !src.text(part).starts_with("//!") => {},
            "block_comment" if !src.text(part).starts_with("/*!") => {},
            _ => break,
        }
        previous = part.prev_sibling();
    }
    result.reverse();
    result
}
