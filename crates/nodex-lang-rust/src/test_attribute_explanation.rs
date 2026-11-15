//! Explain Rust's direct `#[test]` function attribute.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_item" || node.has_error() { return Vec::new(); }
    let mut previous = node.prev_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" => {
                if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute") {
                    let path = named_children(attribute).into_iter().next();
                    if path.is_some_and(|path| src.norm(path) == "test") && attribute.child_by_field_name("arguments").is_none() && attribute.child_by_field_name("value").is_none() {
                        result.push(ContractItem {
                            group: "Built-in test attribute".into(),
                            text: "`#[test]` marks the following function as a test for Rust's built-in test harness. When the project is built in test mode, for example by `cargo test`, the harness discovers eligible included test functions and runs those selected by its filters and ignore settings. Merely declaring the function does not execute the test. A normal successful result passes and a panic or error result normally fails; other attributes such as `#[should_panic]` or `#[ignore]` change expectations or selection. The compiler checks the function's required test signature. Runtime-specific attributes such as `#[tokio::test]` are separate attributes whose behavior depends on their own definition.".into(),
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
