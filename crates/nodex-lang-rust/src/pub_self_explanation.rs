use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Explain the explicitly module-private form of Rust visibility.
pub(crate) fn explain(src: Src, node: Node) -> Option<ContractItem> {
    let visibility = named_children(node).into_iter().find(|child| {
        child.kind() == "visibility_modifier" && src.norm(*child).chars().filter(|c| !c.is_whitespace()).collect::<String>() == "pub(self)"
    })?;
    Some(ContractItem {
        group: "Visibility".into(),
        text: "pub(self) makes this item accessible within the current module and its descendant modules. It has the same visibility as leaving the modifier out; it does not make the item public outside this module.".into(),
        span: src.span(visibility),
        type_info: None,
    })
}
