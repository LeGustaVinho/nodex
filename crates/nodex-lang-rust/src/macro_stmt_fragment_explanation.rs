//! Explain the `stmt` matcher fragment in declarative macros.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if declaration.kind() != "macro_definition" { return Vec::new(); }
    let mut found = Vec::new();
    for rule in named_children(declaration).into_iter().filter(|node| node.kind() == "macro_rule") {
        let Some(matcher) = rule.child_by_field_name("left") else { continue };
        if matcher.has_error() { continue; }
        let mut pending = vec![matcher];
        while let Some(node) = pending.pop() {
            if node.kind() == "token_binding_pattern"
                && let (Some(name), Some(fragment)) = (node.child_by_field_name("name"), node.child_by_field_name("type"))
                && src.text(fragment) == "stmt"
            {
                let repeated = std::iter::successors(node.parent(), |parent| parent.parent())
                    .take_while(|ancestor| *ancestor != matcher)
                    .any(|ancestor| ancestor.kind() == "token_repetition_pattern");
                let repetition = if repeated { " Inside this repetition, each match captures a statement according to the repetition operator and any separator." } else { "" };
                found.push(ContractItem {
                    group: "Macro statement fragment".into(),
                    text: format!("In this `macro_rules!` matcher, {}:stmt matches one Rust statement and captures its syntax as the macro metavariable {}. The fragment normally excludes a trailing semicolon; an item statement that requires a semicolon includes it. If a semicolon is written separately in the matcher, it must match separately. Matching captures syntax rather than executing the statement; uses of the metavariable in the transcriber substitute that syntax into the expansion, which is checked and executed only as the resulting code requires.{}", src.norm(name), src.norm(name), repetition),
                    span: src.span(node),
                    type_info: None,
                });
            }
            pending.extend(named_children(node).into_iter().rev());
        }
    }
    found
}
