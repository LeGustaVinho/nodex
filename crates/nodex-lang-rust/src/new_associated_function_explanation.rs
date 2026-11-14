//! Explains the conventional `new` associated function without guessing its body.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return None; }
    let owner = node.parent()?.parent()?;
    if !matches!(owner.kind(), "impl_item" | "trait_item") { return None; }
    let name = node.child_by_field_name("name")?;
    if src.text(name) != "new" { return None; }
    let result = node.child_by_field_name("return_type")?;
    if src.text(result) != "Self" { return None; }
    let params = node.child_by_field_name("parameters")?;
    if named_children(params).iter().any(|child| !child.is_extra() && child.kind() != "attribute_item") { return None; }
    Some(ContractItem {
        group: "Associated function".into(),
        text: "This declares an associated function named new, with no arguments or self receiver. `-> Self` says it returns the implementing type. Rust programmers often use the name new for a function that creates a value, and an inherent function can be called as Type::new(); the name is a convention, not a special constructor keyword. The function's body determines how its result is obtained; a trait function may require qualified syntax at its call site.".into(),
        span: src.span(name),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn selects_associated_zero_argument_new_returning_self() {
        let text = "struct S; impl S { fn new() -> Self { Self } fn new_arg(x: u8) -> Self { Self } fn other(&self) -> Self { Self } fn nested() { fn new() -> Self { loop {} } } } trait Factory { fn new() -> Self; } fn new() -> u8 { 0 }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "new.rs", index: &index };
        let mut pending = vec![tree.root_node()];
        let mut found = Vec::new();
        while let Some(node) = pending.pop() {
            if let Some(item) = explain(src, node) { found.push(item); }
            pending.extend(named_children(node));
        }
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|item| item.text.contains("convention") && item.text.contains("body determines")));
    }
}
