//! Explain `:pat` metavariables in declarative macro matchers.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, definition: Node<'_>) -> Vec<ContractItem> {
    if definition.kind() != "macro_definition" || definition.has_error() { return Vec::new(); }
    let mut output = Vec::new();
    for rule in named_children(definition).into_iter().filter(|node| node.kind() == "macro_rule") {
        let Some(matcher) = rule.child_by_field_name("left") else { continue; };
        let mut stack = vec![matcher];
        while let Some(part) = stack.pop() {
            if part.kind() == "token_binding_pattern" {
                let (Some(name), Some(fragment)) = (part.child_by_field_name("name"), part.child_by_field_name("type")) else { continue; };
                if src.text(fragment) == "pat" {
                    let name = src.norm(name);
                    output.push(ContractItem {
                        group: "Macro pattern fragment".into(),
                        text: format!("`{name}:pat` occurs in a macro rule's matcher. The `$` name {name} captures the syntax of a Rust pattern accepted at that position; `pat` specifies the fragment kind. It does not evaluate a value or match that pattern against a runtime value during macro expansion. The transcriber can use {name} to insert the captured pattern into generated code. A surrounding macro repetition can capture multiple patterns and imposes repetition rules on their use. The edition of the macro definition controls which patterns `pat` accepts: in Rust 2021 and later it includes top-level `|` alternatives; before 2021 it does not. `pat_param` excludes top-level alternatives in every edition."),
                        span: src.span(part), type_info: None,
                    });
                }
                continue;
            }
            stack.extend(named_children(part).into_iter().rev());
        }
    }
    output
}
