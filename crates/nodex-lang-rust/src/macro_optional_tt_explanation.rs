//! Explain an optional token-tree capture in a declarative macro matcher.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, definition: Node<'_>) -> Vec<ContractItem> {
    if definition.kind() != "macro_definition" || definition.has_error() { return Vec::new(); }
    let mut output = Vec::new();
    for rule in named_children(definition).into_iter().filter(|part| part.kind() == "macro_rule") {
        let Some(matcher) = rule.child_by_field_name("left") else { continue; };
        let mut stack = vec![matcher];
        while let Some(node) = stack.pop() {
            if node.kind() == "token_repetition_pattern" && src.text(node).trim_end().ends_with('?') {
                let items: Vec<_> = named_children(node).into_iter().filter(|part| !part.is_extra()).collect();
                if let [binding] = items.as_slice() {
                    if binding.kind() == "token_binding_pattern" {
                        if let (Some(name), Some(fragment)) = (binding.child_by_field_name("name"), binding.child_by_field_name("type")) {
                            if src.text(fragment) == "tt" {
                                let name = src.norm(name);
                                output.push(ContractItem {
                                    group: "Optional macro token tree".into(),
                                    text: format!("In the macro matcher `{}`, `$(` and `)` group the repeated matcher, and the trailing `?` allows that group to occur zero or one time. When present, `{name}:tt` captures one token tree: a single token or a balanced group of tokens in matching delimiters. The captured syntax may be used in the expansion; a corresponding expansion repetition must obey the same repetition nesting and count. This is optional syntax at macro expansion time, not a runtime condition. Unlike `*` or `+`, `?` permits no repetition separator.", src.norm(node)),
                                    span: src.span(node), type_info: None,
                                });
                            }
                        }
                    }
                }
            }
            stack.extend(named_children(node).into_iter().rev());
        }
    }
    output
}
