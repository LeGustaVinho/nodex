//! Explain unnamed unit constants on their enclosing scope.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "const_item" || node.has_error() { return None; }
    let name = node.child_by_field_name("name")?;
    let ty = node.child_by_field_name("type")?;
    let value = node.child_by_field_name("value")?;
    if src.text(name) != "_" || src.norm(ty) != "()" || src.norm(value) != "()" { return None; }
    Some(ContractItem {
        group: "Unnamed unit constant".into(),
        text: "`const _: () = ();` declares a constant with no usable name: `_` does not introduce a name that can be referenced. The type `()` is unit, and the initializer `()` is its sole value. Multiple unnamed constants may appear in the same scope without name collisions. A constant initializer is checked as a constant expression; this trivial initializer supplies unit and does not itself perform a runtime operation. Other unnamed constants can use more complex initializers subject to constant-evaluation rules.".into(),
        span: src.span(node), type_info: None,
    })
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "const_item" { return describe(src, node).into_iter().collect(); }
    if !matches!(node.kind(), "source_file" | "mod_item" | "function_item" | "impl_item" | "trait_item") { return Vec::new(); }
    let mut pending = vec![node];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if part != node && matches!(part.kind(), "function_item" | "mod_item" | "impl_item" | "trait_item" | "token_tree") { continue; }
        if part.kind() == "const_item" { result.extend(describe(src, part)); continue; }
        pending.extend(named_children(part).into_iter().rev());
    }
    result
}
