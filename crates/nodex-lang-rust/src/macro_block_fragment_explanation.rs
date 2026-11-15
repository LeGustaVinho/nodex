//! Explain `$name:block` fragments in declarative macro matchers.
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
                    && src.text(fragment) == "block"
                {
                    let name = src.norm(name);
                    result.push(ContractItem {
                        group: "Macro block fragment".into(),
                        text: format!("In this macro rule's matcher, {name}:block captures input syntax that parses as a block expression, such as `{{ statements; value }}`, and names it {name}. This is a macro metavariable for syntax, not an ordinary runtime variable. When this rule matches, its template can refer to the captured block as {name}; if the matcher is inside a repetition, the template must use captures according to the repetition rules. Matching alone does not execute the block: any execution depends on the Rust code produced by expansion."),
                        span: src.span(part),
                        type_info: None,
                    });
                }
                continue;
            }
            // Traverse only the rule's matcher, including nested repetitions. The
            // transcriber is deliberately never searched for binding declarations.
            pending.extend(named_children(part).into_iter().rev());
        }
    }
    result
}
