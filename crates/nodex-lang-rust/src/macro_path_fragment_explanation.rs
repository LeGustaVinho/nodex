//! Explain `:path` metavariables in declarative macro matchers.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if declaration.kind() != "macro_definition" { return Vec::new(); }
    let mut items = Vec::new();
    for rule in named_children(declaration).into_iter().filter(|item| item.kind() == "macro_rule" && !item.has_error()) {
        let Some(matcher) = rule.child_by_field_name("left") else { continue; };
        let mut pending = vec![matcher];
        while let Some(part) = pending.pop() {
            if part.kind() == "token_binding_pattern" {
                if let (Some(name), Some(fragment)) = (part.child_by_field_name("name"), part.child_by_field_name("type")) {
                    if src.text(fragment) == "path" {
                        let meta = src.norm(name);
                        items.push(ContractItem {
                            group: "Macro path fragment".into(),
                            text: format!("In the matcher, {meta}:path accepts an input fragment with Rust's type-path syntax and binds that syntax to the macro metavariable {meta}. A path can contain names separated by :: and can include generic arguments. The dollar sign marks a macro metavariable, not an ordinary runtime variable. A matching rule may use {meta} in its expansion template; the matcher itself neither resolves the path to a particular item nor calls it. Name resolution and the position of the expanded syntax determine its meaning. Inside a repetition, a capture is made for each matched iteration."),
                            span: src.span(part),
                            type_info: None,
                        });
                    }
                }
            }
            pending.extend(named_children(part).into_iter().filter(|child| !child.is_extra()).rev());
        }
    }
    items
}
