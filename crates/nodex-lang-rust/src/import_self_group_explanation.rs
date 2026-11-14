use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Explain `use path::{self, item}` without assuming module-path syntax is known.
pub(crate) fn explain(src: Src, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "use_declaration" { return None; }
    let arg = node.child_by_field_name("argument")?;
    if arg.kind() != "scoped_use_list" { return None; }
    let path = arg.child_by_field_name("path")?;
    let list = arg.child_by_field_name("list")?;
    let entries: Vec<_> = named_children(list).into_iter().filter(|child| !child.is_extra()).collect();
    if !entries.iter().any(|child| src.norm(*child) == "self") { return None; }
    let others: Vec<_> = entries.iter().filter(|child| src.norm(**child) != "self")
        .map(|child| src.norm(*child)).collect();
    if others.is_empty() { return None; }
    Some(ContractItem {
        group: "Grouped import".into(),
        text: format!("The shared path {} applies to each entry inside the braces. Here self imports the module {} itself; the remaining entries ({}) import items from that module into the current scope, subject to visibility. This makes the names available without repeating the shared path.", src.norm(path), src.norm(path), others.join(", ")),
        span: src.span(node),
        type_info: None,
    })
}
