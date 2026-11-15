//! Explain the pat_param fragment specifier in macro_rules! matchers.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Inspect matcher syntax only: the same text in an expansion is not a matcher.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "macro_definition" { return Vec::new(); }
    let mut items = Vec::new();
    for rule in named_children(node).into_iter().filter(|part| part.kind() == "macro_rule") {
        let Some(matcher) = rule.child_by_field_name("left") else { continue; };
        let mut pending = vec![matcher];
        while let Some(part) = pending.pop() {
            if part.kind() == "token_binding_pattern" && !part.has_error() {
                if let (Some(name), Some(fragment)) = (part.child_by_field_name("name"), part.child_by_field_name("type"))
                    && src.text(fragment) == "pat_param"
                {
                    items.push(ContractItem {
                        group: "Macro pattern fragment without top-level alternatives".into(),
                        text: format!("{}:pat_param captures a Rust pattern from the macro invocation as syntax and binds it to {} for use in the expansion. pat_param matches a pattern without a top-level | alternative: for example, a rule may use | as a separator outside the captured pattern. An alternative nested inside another pattern can still be part of the capture. The matcher describes syntax during macro expansion; it does not itself execute the pattern or evaluate a value. Unlike pat, whose top-level alternative behavior depends on the macro definition's Rust edition, pat_param consistently excludes a top-level alternative.", src.norm(name), src.norm(name)),
                        span: src.span(fragment),
                        type_info: None,
                    });
                }
                continue;
            }
            pending.extend(named_children(part).into_iter().rev());
        }
    }
    items
}
