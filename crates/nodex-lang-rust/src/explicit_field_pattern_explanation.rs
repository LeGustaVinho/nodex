//! Explain explicit field-to-pattern mappings such as `Point { x: a }`.
use nodex_ir::{Span, reading::ReadingContent};
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Span, Node<'t>, ReadingContent)> {
    if node.kind() != "field_pattern" || node.has_error() { return None; }
    let field = node.child_by_field_name("name")?;
    let pattern = node.child_by_field_name("pattern")?;
    if pattern.kind() != "identifier" { return None; }
    let colon = children(node).into_iter().find(|child| child.kind() == ":" && !child.is_missing())?;
    // A pattern in a match arm attaches to the match header, because arms do not
    // have separate flow nodes. Let declarations attach to their own flow node.
    let mut current = node.parent();
    let mut in_arm = false;
    let anchor = loop {
        let ancestor = current?;
        match ancestor.kind() {
            "match_arm" => in_arm = true,
            "match_expression" if in_arm => break src.header_span(ancestor, ancestor.child_by_field_name("body")),
            "let_declaration" if !in_arm => break src.span(ancestor),
            "function_item" | "closure_expression" | "async_block" => return None,
            _ => {},
        }
        current = ancestor.parent();
    };
    let field_name = src.norm(field);
    let binding = src.norm(pattern);
    Some((anchor, colon, ReadingContent::Detail {
        label: "Map a named field to a pattern".into(),
        text: format!("In {field_name}: {binding}, the name before the colon selects the field {field_name}; the part after the colon is a pattern matched against that field's value. If {binding} introduces a new variable, it binds that field value under the name {binding}, which may differ from the field name. A bare name can instead resolve to an existing constant or variant, so name resolution determines whether it binds or checks a value. Depending on the full pattern and type, matching may copy, move, or borrow the field."),
        definition: None,
        borrow: false,
    }))
}
