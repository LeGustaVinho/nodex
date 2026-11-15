//! Explain two invalid numeric tuple-field spellings retained by the parser.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "field_expression" { return None; }
    let field = node.child_by_field_name("field")?;
    if field.kind() != "integer_literal" { return None; }
    let (reason, suggestion) = match src.text(field) {
        "01" => ("a leading zero", ".1"),
        "0usize" => ("the `usize` suffix", ".0"),
        _ => return None,
    };
    Some((field, ReadingContent::Detail {
        label: "Invalid tuple-field spelling".into(),
        text: format!("`{}` uses {reason} after the dot. A tuple or tuple-struct field index must be written as decimal digits without leading zeros, underscores, or a type suffix. Thus this spelling is invalid; if the intended position was the corresponding numeric field, write `{suggestion}` instead. The receiver's actual type and whether that field exists and is accessible still require compiler checking. This explanation covers the spelling here, not every possible invalid field access.", src.text(node)),
        definition: None, borrow: false,
    }))
}
