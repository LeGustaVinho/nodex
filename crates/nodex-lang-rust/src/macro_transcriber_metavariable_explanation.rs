//! Explain uses of captured metavariables in declarative macro transcribers.
use std::collections::HashSet;
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "macro_definition" || node.has_error() { return Vec::new(); }
    let mut result = Vec::new();
    for rule in named_children(node).into_iter().filter(|part| part.kind() == "macro_rule" && !part.has_error()) {
        let (Some(matcher), Some(transcriber)) = (rule.child_by_field_name("left"), rule.child_by_field_name("right")) else { continue; };
        let mut bound = HashSet::new();
        let mut pending = vec![matcher];
        while let Some(part) = pending.pop() {
            if part.kind() == "token_binding_pattern" {
                if let Some(name) = part.child_by_field_name("name") { bound.insert(src.norm(name)); }
                continue;
            }
            pending.extend(named_children(part));
        }
        pending.push(transcriber);
        while let Some(part) = pending.pop() {
            if part.kind() == "metavariable" {
                let name = src.norm(part);
                if name != "$crate" && bound.contains(&name) {
                    result.push(ContractItem {
                        group: "Macro expansion metavariable".into(),
                        text: format!("In the expansion template after `=>`, {name} refers to syntax captured under the same name by this rule's matcher. When the rule matches an invocation, macro expansion substitutes the captured syntax here; {name} is a macro metavariable, not a runtime variable. In a repetition, substitution must follow the matcher and transcriber's repetition rules. Substitution itself does not evaluate an expression: the resulting Rust code determines later behavior."),
                        span: src.span(part), type_info: None,
                    });
                }
                continue;
            }
            if part.is_extra() || matches!(part.kind(), "string_literal" | "raw_string_literal" | "char_literal" | "line_comment" | "block_comment") { continue; }
            pending.extend(named_children(part).into_iter().rev());
        }
    }
    result
}
