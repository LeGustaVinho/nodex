//! Explain the concrete empty-input and literal-x declarative macro rules.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "macro_definition" || node.has_error() { return None; }
    let name = node.child_by_field_name("name")?;
    let rules: Vec<_> = named_children(node).into_iter().filter(|part| part.kind() == "macro_rule").collect();
    if rules.len() != 2 { return None; }
    let (first_match, first_result) = (rules[0].child_by_field_name("left")?, rules[0].child_by_field_name("right")?);
    let (second_match, second_result) = (rules[1].child_by_field_name("left")?, rules[1].child_by_field_name("right")?);
    if src.norm(first_match) != "()" || src.norm(second_match) != "(x)" || src.norm(first_result) != "{}" || src.norm(second_result) != "{}" { return None; }
    Some(ContractItem {
        group: "Macro with empty and literal-token rules".into(),
        text: format!("`macro_rules! {}` defines two rules. The compiler tries them in order: the matcher `()` accepts no input tokens, while `(x)` accepts the literal token `x`; it does not bind an arbitrary variable named x. Thus `{}!()` selects the first rule and `{}!(x)` the second. In each rule `=>` separates the matcher from the transcriber; `{{}}` is an empty transcriber, so it emits no tokens, not an empty Rust block. The semicolon separates rules. An invocation with different input fails to match these rules. The outer matcher delimiters can match any invocation delimiter pair. Whether the resulting empty expansion is valid depends on the invocation's syntactic context; if the first matching expansion fails, later rules are not tried.", src.norm(name), src.norm(name), src.norm(name)),
        span: src.span(node), type_info: None,
    })
}
