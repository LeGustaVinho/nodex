use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src, node: Node<'_>) -> Option<ReadingContent> {
    if node.kind() != "type_cast_expression" || node.has_error() { return None; }
    let value = node.child_by_field_name("value")?;
    let target = node.child_by_field_name("type")?;
    children(node).into_iter().find(|token| token.kind() == "as" && !token.is_missing())?;
    Some(ReadingContent::Detail {
        label: "Explicit type conversion".into(),
        text: format!("Evaluate {}, then convert its value to {} using Rust's explicit `as` cast. Rust permits only specific conversions; the result and possible loss of information depend on the source and target types. This is a cast, unlike C#'s `as` operator for safe reference conversion.", src.norm(value), src.norm(target)),
        definition: None,
        borrow: false,
    })
}
