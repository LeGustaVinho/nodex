//! Explain the bare `#[ignore]` test attribute on the following function.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_item" || node.has_error() { return Vec::new(); }
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
        if attribute.child_by_field_name("arguments").is_some() || attribute.child_by_field_name("value").is_some() { continue; }
        let Some(path) = named_children(attribute).into_iter().next() else { continue; };
        if src.text(path) != "ignore" { continue; }
        output.push(ContractItem {
            group: "Ignored test attribute".into(),
            text: "`#[ignore]` is an outer attribute on the following function. When the function is a test using Rust's standard test harness, it is listed as ignored and skipped by a normal test run rather than executed. Run ignored tests with `cargo test -- --ignored`, or include them with `cargo test -- --include-ignored`. The attribute does not delete the test or guarantee a particular result; behavior with a custom test harness can differ.".into(),
            span: src.span(part), type_info: None,
        });
    }
    output
}
