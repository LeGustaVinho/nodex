//! Explain a named-field pattern with an inclusive range and omitted fields.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "struct_pattern" || node.has_error() { return None; }
    let fields = named_children(node);
    let rest = fields.iter().copied().find(|field| field.kind() == "remaining_field_pattern")?;
    let field = fields.iter().copied().find(|field| {
        field.kind() == "field_pattern" && field.child_by_field_name("pattern").is_some_and(|pattern| pattern.kind() == "range_pattern" && src.text(pattern).contains("..="))
    })?;
    let field_name = field.child_by_field_name("name")?;
    let range = field.child_by_field_name("pattern")?;
    let path = node.child_by_field_name("type")?;
    Some((rest, ReadingContent::Detail {
        label: "Check a named field and ignore the rest".into(),
        text: format!("This pattern checks a value against the named-field shape {}. Its {} field must match the inclusive range {}: both written endpoints are included. Any other explicitly written field patterns must match too. The .. inside the braces means the remaining fields are not individually listed or bound; it does not create or change a value. The pattern succeeds only when the shape and all written checks match. The path may identify a struct or a named-field enum variant, depending on name resolution.", src.norm(path), src.norm(field_name), src.norm(range)),
        definition: None,
        borrow: false,
    }))
}
