//! Explain numbered fields in struct update expressions.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'a>(src: Src<'_>, node: Node<'a>) -> Option<(Node<'a>, ReadingContent)> {
    if node.kind() != "struct_expression" || node.has_error() { return None; }
    let name = node.child_by_field_name("name")?;
    let fields = named_children(node.child_by_field_name("body")?);
    let base = fields.iter().find(|f| f.kind() == "base_field_initializer")?;
    let base_value = named_children(*base).into_iter().find(|n| !n.is_extra())?;
    let numbered = fields.iter().find_map(|f| {
        if f.kind() != "field_initializer" { return None; }
        let field = f.child_by_field_name("field")?;
        let text = src.norm(field);
        if !text.chars().all(|c| c.is_ascii_digit()) || text.is_empty() { return None; }
        Some((field, f.child_by_field_name("value")?))
    })?;
    Some((numbered.0, ReadingContent::Detail {
        label: "Update a numbered field".into(),
        text: format!("{} names the value to construct. {} selects a positional field by its zero-based index; the expression {} supplies that field's new value. The ..{} base supplies fields not explicitly written. Those fields are copied if their types implement Copy, otherwise moved. The declaration determines whether this construction is valid and what its fields mean.", src.norm(name), src.norm(numbered.0), src.norm(numbered.1), src.norm(base_value)),
        definition: None,
        borrow: false,
    }))
}
