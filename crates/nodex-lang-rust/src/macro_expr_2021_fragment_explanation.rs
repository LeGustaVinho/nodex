//! Explain the `expr_2021` matcher fragment in declarative macros.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if declaration.kind() != "macro_definition" { return Vec::new(); }
    let mut found = Vec::new();
    for rule in named_children(declaration).into_iter().filter(|node| node.kind() == "macro_rule") {
        let Some(matcher) = rule.child_by_field_name("left") else { continue };
        if matcher.has_error() { continue; }
        // Only descend into the matcher, never the transcriber on the right of =>.
        let mut pending = vec![matcher];
        while let Some(node) = pending.pop() {
            if node.kind() == "token_binding_pattern"
                && let (Some(name), Some(fragment)) = (node.child_by_field_name("name"), node.child_by_field_name("type"))
                && src.text(fragment) == "expr_2021"
            {
                let repeated = std::iter::successors(node.parent(), |parent| parent.parent())
                    .take_while(|ancestor| *ancestor != matcher)
                    .any(|ancestor| ancestor.kind() == "token_repetition_pattern");
                let repetition = if repeated { " This capture appears inside a matcher repetition: it is matched for each repetition allowed by that repetition's operator and separator." } else { "" };
                found.push(ContractItem {
                    group: "Macro expr_2021 fragment".into(),
                    text: format!("In this `macro_rules!` matcher, {}:expr_2021 captures the syntax of an expression into the macro metavariable {}. It matches the older expression fragment rules: a top-level `_` expression or `const {{ ... }}` block is not matched, although such forms can occur inside a larger expression. In Rust 2024, `expr` also accepts those top-level forms, while `expr_2021` retains the earlier restriction for compatibility. The edition relevant to `expr` is the edition of the macro definition, not the call site. Matching captures syntax; it does not evaluate the expression.{}", src.norm(name), src.norm(name), repetition),
                    span: src.span(node),
                    type_info: None,
                });
            }
            pending.extend(named_children(node).into_iter().rev());
        }
    }
    found
}
