//! Explain type fragments in declarative macro matchers.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if declaration.kind() != "macro_definition" || declaration.has_error() { return Vec::new(); }
    let mut result = Vec::new();
    for rule in named_children(declaration).into_iter().filter(|part| part.kind() == "macro_rule") {
        let Some(matcher) = rule.child_by_field_name("left") else { continue; };
        let mut pending = vec![(matcher, false)];
        while let Some((part, repeated)) = pending.pop() {
            let repeated = repeated || part.kind() == "token_repetition_pattern";
            if part.kind() == "token_binding_pattern" {
                let (Some(name), Some(fragment)) = (part.child_by_field_name("name"), part.child_by_field_name("type")) else { continue; };
                if src.text(fragment) != "ty" { continue; }
                let repetition = if repeated { " This capture is inside a repetition: each repetition can capture another type, and the expansion must follow macro repetition rules." } else { "" };
                result.push(ContractItem {
                    group: "Macro type fragment".into(),
                    text: format!("In a `macro_rules!` matcher, {}:ty captures input that parses as a Rust type and names the captured syntax {}. `ty` is a fragment specifier: it describes the kind of syntax accepted, not a runtime value or a variable declaration. The macro can substitute {} in its expansion template; matching captures syntax rather than evaluating a value. The compiler still resolves and checks the expanded type in its context.{} Text in the output template is not a matcher declaration.", src.norm(name), src.norm(name), src.norm(name), repetition),
                    span: src.span(part), type_info: None,
                });
                continue;
            }
            pending.extend(named_children(part).into_iter().rev().map(|child| (child, repeated)));
        }
    }
    result
}
