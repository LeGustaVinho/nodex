//! Explain numeric positional field selection, including tuple structs.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "field_expression" || node.has_error() { return None; }
    let base = node.child_by_field_name("value")?;
    let field = node.child_by_field_name("field")?;
    let index: usize = src.text(field).parse().ok()?;
    if index != 1 { return None; }
    Some((field, ReadingContent::Detail {
        label: "Select a positional field".into(),
        text: format!("In {}, the dot and {} select the second positional field of {}. Numbering starts at zero, so .0 is first and .1 is second. This is a statically written field number, not a runtime array index. The compiler checks that the selected field exists on the tuple or tuple struct (possibly through reference dereferencing). The surrounding context determines whether the field is read, borrowed, assigned, copied, or moved; this syntax alone does not prove which occurs.", src.norm(node), src.norm(field), src.norm(base)),
        definition: None, borrow: false,
    }))
}
