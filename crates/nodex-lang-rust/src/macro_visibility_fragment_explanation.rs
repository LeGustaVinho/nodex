//! Explain `:vis` metavariables in macro_rules! matchers.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Only traverse matchers: transcribers contain output tokens rather than
/// metavariable declarations. Nested repetition groups are traversed too.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "macro_definition" { return Vec::new(); }
    let mut output = Vec::new();
    for rule in named_children(node).into_iter().filter(|part| part.kind() == "macro_rule" && !part.has_error()) {
        let Some(matcher) = rule.child_by_field_name("left") else { continue; };
        let mut pending = vec![matcher];
        while let Some(part) = pending.pop() {
            if part.kind() == "token_binding_pattern" {
                if let (Some(name), Some(kind)) = (part.child_by_field_name("name"), part.child_by_field_name("type")) {
                    if src.text(kind) == "vis" {
                        output.push(ContractItem {
                            group: "Macro visibility fragment".into(),
                            text: format!("In this matcher, {}:vis declares a metavariable that captures Rust visibility syntax, such as `pub` or `pub(crate)`. The fragment can also match no visibility tokens, so it does not by itself make an item public. The captured syntax is available as {} in the transcriber, where it can be inserted into the generated declaration. Inside a repetition, the surrounding sequence and repetition operator control how often the matcher repeats; a repeated matcher must make progress and cannot consist solely of a fragment that matches empty input.", src.norm(name), src.norm(name)),
                            span: src.span(part), type_info: None,
                        });
                    }
                }
                continue;
            }
            pending.extend(named_children(part).into_iter().rev());
        }
    }
    output
}
