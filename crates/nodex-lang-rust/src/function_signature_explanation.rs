//! Beginner-facing explanation of Rust's parameter and return-type syntax.

use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") {
        return None;
    }
    let name = node.child_by_field_name("name")?;
    let result = node.child_by_field_name("return_type")?;
    let parameters = node.child_by_field_name("parameters")?;
    let mut descriptions = Vec::new();
    for parameter in named_children(parameters).into_iter().filter(|p| !p.is_extra() && p.kind() != "attribute_item") {
        if parameter.kind() != "parameter" { return None; }
        let pattern = parameter.child_by_field_name("pattern")?;
        let ty = parameter.child_by_field_name("type")?;
        descriptions.push(format!("{} has type {}", src.norm(pattern), src.norm(ty)));
    }
    if descriptions.is_empty() { return None; }
    Some(ContractItem {
        group: "Function signature".into(),
        text: format!("fn declares the function {}. The parentheses list its inputs: {}. In each input, the type follows the name or pattern after a colon. The arrow -> introduces the function's return type, {}.", src.norm(name), descriptions.join("; "), src.norm(result)),
        span: src.header_span(node, node.child_by_field_name("body")),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn explains_inputs_and_result_for_a_function_and_a_trait_signature() {
        let text = "fn foo(x: i32) -> i32 { x } trait T { fn bar(x: u8) -> bool; }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "signature.rs", index: &index };
        let mut pending = vec![tree.root_node()];
        let mut explanations = Vec::new();
        while let Some(node) = pending.pop() {
            if let Some(item) = explain(src, node) { explanations.push(item.text); }
            pending.extend(named_children(node));
        }
        assert_eq!(explanations.len(), 2);
        assert!(explanations.iter().any(|text| text.contains("x has type i32") && text.contains("return type, i32")));
        assert!(explanations.iter().any(|text| text.contains("x has type u8") && text.contains("return type, bool")));
    }
}
