//! Explain a macro_rules expression matcher with a square-delimited transcriber.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "macro_definition" { return Vec::new(); }
    named_children(node).into_iter().filter_map(|rule| {
        if rule.kind() != "macro_rule" || rule.has_error() { return None; }
        let matcher = rule.child_by_field_name("left")?;
        let transcriber = rule.child_by_field_name("right")?;
        if !src.text(matcher).trim_start().starts_with('(') || !src.text(transcriber).trim_start().starts_with('[') { return None; }
        let parts: Vec<_> = named_children(matcher).into_iter().filter(|part| !part.is_extra()).collect();
        let [binding] = parts.as_slice() else { return None; };
        if binding.kind() != "token_binding_pattern" { return None; }
        let name = binding.child_by_field_name("name")?;
        let fragment = binding.child_by_field_name("type")?;
        if src.text(fragment) != "expr" { return None; }
        Some(ContractItem {
            group: "Macro rule with square-delimited expansion".into(),
            text: format!("This macro_rules! rule matches an expression as syntax and binds it to {}: the $ introduces a metavariable, and :expr specifies the expression fragment. The => separates the matcher from the expansion template. The square brackets around the template delimit the macro transcriber; they are not themselves emitted as array brackets. Uses of {} inside the template are replaced by the captured expression's syntax during macro expansion. To emit square brackets as part of the result, include them inside the transcriber, for example => {{ [{}] }}. The expression is evaluated only as required by the code produced after expansion; the matcher does not evaluate it.", src.norm(name), src.norm(name), src.norm(name)),
            span: src.span(rule),
            type_info: None,
        })
    }).collect()
}
