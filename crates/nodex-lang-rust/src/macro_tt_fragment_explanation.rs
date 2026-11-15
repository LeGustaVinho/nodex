//! Explain `$name:tt` fragments in declarative macro matchers.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if declaration.kind() != "macro_definition" { return Vec::new(); }
    let mut result = Vec::new();
    for rule in named_children(declaration).into_iter().filter(|node| node.kind() == "macro_rule" && !node.has_error()) {
        let Some(matcher) = rule.child_by_field_name("left") else { continue; };
        let mut pending = vec![matcher];
        while let Some(part) = pending.pop() {
            if part.kind() == "token_binding_pattern" {
                if let (Some(name), Some(fragment)) = (part.child_by_field_name("name"), part.child_by_field_name("type"))
                    && src.text(fragment) == "tt"
                {
                    let name = src.norm(name);
                    result.push(ContractItem {
                        group: "Macro token tree fragment".into(),
                        text: format!("In this rule's matcher, {name}:tt captures one token tree and names that syntax {name}. A token tree can be a single token or a matched delimiter pair with its contents, including nested groups: for example `(a + b)` is one grouped token tree, while ungrouped `a + b` consists of multiple tokens. The template can refer to the captured syntax as {name}; captures inside repetitions must follow the repetition rules. This is macro syntax, not a runtime variable, and matching it does not evaluate the captured tokens."),
                        span: src.span(part),
                        type_info: None,
                    });
                }
                continue;
            }
            // Only search the matcher, including nested repetitions; the rule's
            // transcriber is not a declaration of fragment bindings.
            pending.extend(named_children(part).into_iter().rev());
        }
    }
    result
}
