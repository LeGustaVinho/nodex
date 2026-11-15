//! Explain direct outer `#[allow(dead_code)]` lint attributes.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "enum_variant" | "union_item" | "trait_item" | "type_item" | "associated_type" | "const_item" | "static_item" | "field_declaration" | "mod_item" | "macro_definition" | "impl_item") { return Vec::new(); }
    let mut previous = node.prev_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" => {
                if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute") {
                    let path = named_children(attribute).into_iter().next();
                    let arguments = attribute.child_by_field_name("arguments");
                    let dead_code = arguments.is_some_and(|arguments| {
                        let tokens: Vec<_> = children(arguments).into_iter().filter(|child| !child.is_extra()).collect();
                        tokens.windows(3).any(|parts| {
                            matches!(parts[0].kind(), "(" | ",") && parts[1].kind() == "identifier"
                                && src.text(parts[1]) == "dead_code" && matches!(parts[2].kind(), ")" | ",")
                        })
                    });
                    if path.is_some_and(|path| src.norm(path) == "allow") && dead_code {
                        result.push(ContractItem {
                            group: "Allow dead-code lint".into(),
                            text: "`#[allow(dead_code)]` sets the `dead_code` lint to allow for the following item and its nested scope, so the compiler normally suppresses that lint's warning about unused, unexported code there. It does not remove the code, run it, or change its runtime behavior. Lint settings can be affected by enclosing scopes and command-line options; a higher `forbid` cannot normally be lowered by this allow. Other lints remain governed by their own levels.".into(),
                            span: src.span(part), type_info: None,
                        });
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
    result
}
