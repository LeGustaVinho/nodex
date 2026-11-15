//! Explain `#[global_allocator]` on a static item without guessing type resolution.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "static_item" || node.has_error() { return Vec::new(); }
    let (Some(name), Some(ty)) = (node.child_by_field_name("name"), node.child_by_field_name("type")) else { return Vec::new(); };
    let mut previous = node.prev_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" => {
                if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute") {
                    let path = named_children(attribute).into_iter().next();
                    if path.is_some_and(|path| src.norm(path) == "global_allocator") && attribute.child_by_field_name("arguments").is_none() && attribute.child_by_field_name("value").is_none() {
                        result.push(ContractItem {
                            group: "Global allocator selection".into(),
                            text: format!("`#[global_allocator]` selects the following static {} as the program's global allocator, used for default allocation requests such as those made by `Box` and `Vec`. Rust requires the attribute on a static whose type implements `GlobalAlloc`, and it may appear only once in the crate graph including dependencies. The written type here is {}; the source spelling alone does not prove that it implements the trait or that another allocator is absent. The allocator implementation must uphold its allocation safety contracts. This does not guarantee that every apparent allocation makes a call to it: optimization, other allocators, and some runtime internals can behave differently.", src.norm(name), src.norm(ty)),
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
