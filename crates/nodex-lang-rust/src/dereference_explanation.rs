//! Explain the unary dereference operator without guessing the operand's type.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

/// Returns the `*` token and a syntax-based explanation for a dereference expression.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "unary_expression" || node.has_error() { return None; }
    let operator = children(node).into_iter().find(|child| child.kind() == "*" && !child.is_missing())?;
    let operand = named_children(node).into_iter().next()?;
    Some((operator, ReadingContent::Detail {
        label: "Dereference a value".into(),
        text: format!("The unary * accesses the place reached through {}. If it is a reference, this accesses the referenced value; some other types can define dereference behavior through Deref or DerefMut. A raw pointer may also be dereferenced, but doing so requires unsafe and its validity and access rules must be upheld. The surrounding expression decides whether to read, borrow, or assign to the place; an assignment requires permission to mutate it. Reading may copy or move a value only when Rust's rules allow it. The operand's exact type and operation cannot be determined from * alone.", src.norm(operand)),
        definition: None,
        borrow: false,
    }))
}
