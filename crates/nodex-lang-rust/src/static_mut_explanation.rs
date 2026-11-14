use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Explain only a mutable static declaration; ordinary statics and local `let mut`s differ.
pub(crate) fn explain(src: Src, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "static_item" {
        return None;
    }
    let mutability = children(node).into_iter().find(|child| child.kind() == "mutable_specifier")?;
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "Mutable static".into(),
        text: format!(
            "{} is one mutable value stored for the entire program, shared by its accesses. Its initializer supplies its starting value. Reading or writing a mutable static requires unsafe code: the programmer must prevent data races and invalid aliasing, including across threads. Prefer a synchronization primitive when sharing mutable state.",
            src.norm(name),
        ),
        span: src.span(mutability),
        type_info: None,
    })
}
