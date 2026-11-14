//! Explain one positional constructor pattern containing a nested tuple pattern.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

/// Match `Name((a, b))` in a match arm without guessing the identity of Name
/// or whether a syntactic identifier denotes a new binding or a constant.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "tuple_struct_pattern" || node.has_error() { return None; }
    let name = node.child_by_field_name("type")?;
    let fields: Vec<_> = named_children(node).into_iter().filter(|part| *part != name).collect();
    if fields.len() != 1 || fields[0].kind() != "tuple_pattern" { return None; }
    let tuple = fields[0];
    let items = named_children(tuple);
    if items.len() < 2 || items.iter().any(|item| item.kind() != "identifier") { return None; }
    let comma = children(tuple).into_iter().find(|part| part.kind() == "," && !part.is_missing())?;
    let arm = std::iter::successors(node.parent(), |parent| parent.parent())
        .take_while(|parent| !matches!(parent.kind(), "function_item" | "closure_expression"))
        .find(|parent| parent.kind() == "match_arm")?;
    let enclosing = arm.parent()?.parent()?;
    if enclosing.kind() != "match_expression" { return None; }
    let projections = items.iter().enumerate().map(|(index, item)| format!("{} matches tuple position .{index}", src.norm(*item))).collect::<Vec<_>>().join("; ");
    Some((enclosing, comma, ReadingContent::Detail {
        label: "Match a nested tuple pattern".into(),
        text: format!("{} is the name of a positional constructor or variant in the outer pattern. Its one positional field must match the inner tuple pattern: {}. If an inner name is a new binding, it receives that position for this arm; names can instead resolve to constants, so the syntax alone does not establish that every name binds. Values may be moved, copied, or borrowed depending on their types and the matching context. For Some, name resolution determines whether this is the standard Option variant or another declaration; the spelling alone is not proof. If this pattern does not match, Rust considers the next arm.", src.norm(name), projections),
        definition: None,
        borrow: false,
    }))
}
