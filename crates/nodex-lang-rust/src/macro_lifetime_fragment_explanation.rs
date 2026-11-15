//! Explain `$name:lifetime` bindings in declarative macro matchers.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "macro_definition" || node.has_error() { return Vec::new(); }
    let mut result = Vec::new();
    for rule in named_children(node).into_iter().filter(|part| part.kind() == "macro_rule") {
        // Only descend into the matcher (left of =>). A similarly written token
        // sequence in the transcriber is not a fragment declaration.
        let Some(matcher) = rule.child_by_field_name("left") else { continue; };
        let mut pending = vec![matcher];
        while let Some(part) = pending.pop() {
            if part.kind() == "token_binding_pattern" {
                if let (Some(name), Some(fragment)) = (part.child_by_field_name("name"), part.child_by_field_name("type"))
                    && src.text(fragment) == "lifetime"
                {
                    let name = src.norm(name);
                    result.push(ContractItem {
                        group: "Macro lifetime fragment".into(),
                        text: format!("In this macro matcher, {name}:lifetime captures one lifetime fragment from the invocation, such as 'a, and binds its syntax to the macro metavariable {name}. The part after `:` specifies what kind of syntax matches; {name} is not a runtime variable. A matching rule can substitute the captured lifetime syntax in its expansion. Inside a repetition, the matcher can capture one lifetime per repetition, subject to macro repetition rules. Matching or substituting the lifetime does not create a borrow or extend the life of data; the expanded Rust code is checked separately."),
                        span: src.span(part), type_info: None,
                    });
                }
                continue;
            }
            pending.extend(named_children(part).into_iter().rev());
        }
    }
    result
}
