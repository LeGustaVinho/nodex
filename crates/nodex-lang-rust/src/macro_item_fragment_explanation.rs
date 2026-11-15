//! Explain `:item` metavariables in macro_rules! matchers.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Traverse only each rule's matcher, including nested repetition groups; a
/// transcriber is a template rather than another matcher.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "macro_definition" { return Vec::new(); }
    let mut output = Vec::new();
    for rule in named_children(node).into_iter().filter(|part| part.kind() == "macro_rule" && !part.has_error()) {
        let Some(matcher) = rule.child_by_field_name("left") else { continue; };
        let mut pending = vec![matcher];
        while let Some(part) = pending.pop() {
            if part.kind() == "token_binding_pattern" {
                if let (Some(name), Some(kind)) = (part.child_by_field_name("name"), part.child_by_field_name("type")) {
                    if src.text(kind) == "item" {
                        output.push(ContractItem {
                            group: "Macro item fragment".into(),
                            text: format!("In this matcher, {}:item declares a macro metavariable: `item` requires one complete Rust item as input syntax, such as a function, struct, constant, or module declaration. The matched syntax is captured as {}; the matcher does not execute that declaration or evaluate it. A use of {} in the transcriber inserts the captured syntax into the expansion. If the binding is inside a repetition such as `$({}:item)*`, the repetition operator controls how many items are matched; the expansion must respect the repetition structure.", src.norm(name), src.norm(name), src.norm(name), src.norm(name)),
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
