//! Explain the safety contract carried by an unsafe trait declaration.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "trait_item" { return None; }
    let keyword = children(node).into_iter().find(|child| child.kind() == "unsafe")?;
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "Unsafe trait".into(),
        text: format!("`unsafe trait {}` declares a trait whose implementers must uphold safety requirements documented by that trait. Writing `unsafe impl` asserts that the implementer has checked and will maintain those requirements; unsafe code may rely on them. The exact requirements come from this trait's documentation and cannot be determined from the keyword alone. Declaring the trait does not itself make every method unsafe to call.", src.norm(name)),
        span: src.span(keyword),
        type_info: None,
    })
}
