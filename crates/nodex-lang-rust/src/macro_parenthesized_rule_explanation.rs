//! Explain a simple expression matcher and parenthesized transcriber.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "macro_definition" || node.has_error() { return Vec::new(); }
    let mut items = Vec::new();
    for rule in named_children(node).into_iter().filter(|part| part.kind() == "macro_rule") {
        let (Some(matcher), Some(transcriber)) = (rule.child_by_field_name("left"), rule.child_by_field_name("right")) else { continue; };
        if !src.text(matcher).starts_with('(') || !src.text(transcriber).starts_with('(') { continue; }
        let bindings = named_children(matcher);
        if bindings.len() != 1 || bindings[0].kind() != "token_binding_pattern" { continue; }
        let binding = bindings[0];
        let (Some(name), Some(fragment)) = (binding.child_by_field_name("name"), binding.child_by_field_name("type")) else { continue; };
        if src.text(fragment) != "expr" { continue; }
        let meta = src.norm(name);
        items.push(ContractItem {
            group: "Macro rule with parenthesized expansion".into(),
            text: format!("In this `macro_rules!` rule, `({meta}:expr)` is the matcher: it accepts an expression fragment and binds its syntax to the metavariable {meta}. The arrow `=>` separates the matcher from the transcriber on the right. The outer parentheses on the right delimit the transcriber and are not automatically emitted around its output; occurrences of {meta} inside the transcriber substitute the captured syntax. Macro expansion works on syntax before the resulting code executes; it does not itself evaluate the expression. Rules are tried in order and the first matching rule is expanded."),
            span: src.span(rule), type_info: None,
        });
    }
    items
}
