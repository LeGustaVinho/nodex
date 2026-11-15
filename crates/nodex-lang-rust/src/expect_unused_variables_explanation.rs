//! Explain `#[expect(unused_variables)]` without claiming a lint actually fires.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if matches!(node.kind(), "source_file" | "attribute_item" | "inner_attribute_item") || node.has_error() { return Vec::new(); }
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
        if src.text(path) != "expect" { continue; }
        let tokens: Vec<_> = children(arguments).into_iter().filter(|child| !child.is_extra()).collect();
        let [open, lint, rest @ ..] = tokens.as_slice() else { continue; };
        if open.kind() != "(" || src.text(*lint) != "unused_variables" { continue; }
        if !matches!(rest, [close] if close.kind() == ")") && !matches!(rest, [comma, close] if comma.kind() == "," && close.kind() == ")") { continue; }
        output.push(ContractItem {
            group: "Expected unused-variable lint".into(),
            text: "`#[expect(unused_variables)]` is an outer lint attribute on the following item. It tells Rust to expect an unused-variable lint in that scope and suppress the expected lint's normal diagnostic when it occurs. If no occurrence fulfills the expectation, Rust can report the `unfulfilled_lint_expectations` lint instead, unless lint configuration suppresses or changes that diagnostic. The attribute is a compile-time lint setting; it does not prove that an unused variable exists or change the program's runtime behavior.".into(),
            span: src.span(part), type_info: None,
        });
    }
    output
}
