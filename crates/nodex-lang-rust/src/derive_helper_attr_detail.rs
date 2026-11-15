//! Explain derive procedural macro helper-attribute declarations.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_item" { return Vec::new(); }
    let mut previous = node.prev_named_sibling();
    let mut items = Vec::new();
    while let Some(part) = previous {
        if part.kind() == "attribute_item" && !part.has_error() {
            if let Some(attribute) = named_children(part).into_iter().find(|n| n.kind() == "attribute") {
                let path = named_children(attribute).into_iter().next();
                if path.is_some_and(|n| src.text(n) == "proc_macro_derive") {
                    if let Some(args) = attribute.child_by_field_name("arguments") {
                        let tokens: Vec<_> = named_children(args).into_iter().filter(|n| !n.is_extra()).collect();
                        if let [name, keyword, helpers] = tokens.as_slice() {
                            if matches!(name.kind(), "identifier" | "type_identifier") && src.text(*keyword) == "attributes" && helpers.kind() == "token_tree" {
                                let names: Vec<_> = named_children(*helpers).into_iter().filter(|n| !n.is_extra()).collect();
                                if !names.is_empty() && names.iter().all(|n| n.kind() == "identifier") {
                                    let helper_names = names.iter().map(|n| format!("`#[{}]`", src.norm(*n))).collect::<Vec<_>>().join(", ");
                                    items.push(ContractItem {
                                        group: "Derive helper attributes".into(),
                                        text: format!("In {}, `attributes(...)` declares {} as helper attribute names for the derive macro {}. When that derive is applied to an item, these helpers are in scope on that item after the derive and on its fields and variants. They are inert annotations: they do not run on their own. The derive can inspect them in its input tokens, and other macros can see them too; their exact meaning and effect depend on macro code. This declaration alone does not prove the procedural macro is valid or that any helper is actually used.", src.norm(part), helper_names, src.norm(*name)),
                                        span: src.span(*helpers), type_info: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        } else if !part.is_extra() { break; }
        previous = part.prev_named_sibling();
    }
    items.reverse();
    items
}
