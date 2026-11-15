//! Explain a direct procedural attribute-macro declaration.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_item" || node.has_error() { return Vec::new(); }
    let mut previous = node.prev_sibling();
    let mut items = Vec::new();
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" => {
                if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute") {
                    let path = named_children(attribute).into_iter().next();
                    if path.is_some_and(|path| src.norm(path) == "proc_macro_attribute") && attribute.child_by_field_name("arguments").is_none() {
                        items.push(ContractItem {
                            group: "Procedural attribute macro".into(),
                            text: "`#[proc_macro_attribute]` marks the following function as the implementation of an attribute-like procedural macro, when the compiler accepts it in a proc-macro crate. The required public function receives two token streams: the arguments written in the applied attribute and the item to which the attribute was applied. It returns a token stream that replaces the annotated item in the expanded program. The function runs during compilation; its returned tokens are then parsed and checked in context. The attribute alone does not say what transformation this function performs or guarantee that the input item is preserved.".into(),
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
    items
}
