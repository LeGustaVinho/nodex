//! Explain numeric field access without assuming a resolved receiver type.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "field_expression" || node.has_error() { return None; }
    let receiver = node.child_by_field_name("value")?;
    let field = node.child_by_field_name("field")?;
    if field.kind() != "integer_literal" { return None; }
    let position = src.norm(field);
    if position != "0" { return None; }
    Some((field, ReadingContent::Detail {
        label: "Access a positional field".into(),
        text: format!("In {}, the dot followed by {position} selects field number {position} of {}. Positions start at zero: .0 selects the first field, .1 the second, and so on. Tuples and tuple structs have positional fields; Rust must resolve the receiver's type and check that this field exists and is accessible. The access denotes a place: depending on how it is used and its type, a value can be copied or moved, borrowed, or assigned. This is field access, not decimal arithmetic or array indexing.", src.norm(node), src.norm(receiver)),
        definition: None,
        borrow: false,
    }))
}
