//! Explain identifier fragments in declarative macro matchers.
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
                if src.text(fragment) != "ident" { continue; }
                let repetition = if repeated { " This capture is inside a repetition: it can collect one matching identifier for each repetition; its use in the output must follow macro repetition rules." } else { "" };
                result.push(ContractItem {
                    group: "Macro identifier fragment".into(),
                    text: format!("In a `macro_rules!` matcher, {}:ident captures one identifier-like input token and names the captured syntax {}. `ident` is a fragment specifier, not a Rust type: it can match identifiers, raw identifiers, and eligible keywords under Rust's macro rules. The macro can refer to {} in its expansion template; this captures syntax during macro expansion rather than reading a runtime variable.{} The same text in an output template is not a matcher declaration.", src.norm(name), src.norm(name), src.norm(name), repetition),
                    span: src.span(part), type_info: None,
                });
                continue;
            }
            pending.extend(named_children(part).into_iter().rev().map(|child| (child, repeated)));
        }
    }
    result
}
