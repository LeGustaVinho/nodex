//! Explain shorthand named-field patterns in match arms.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

/// Describe plain shorthand field bindings inside a struct or variant pattern.
/// Restrict to match arms; let destructuring has a separate explanation.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "struct_pattern" || node.has_error() { return None; }
    let owner = node.child_by_field_name("type")?;
    let fields: Vec<_> = named_children(node).into_iter().filter(|child| child.kind() == "field_pattern").collect();
    if fields.is_empty() || fields.iter().any(|field| {
        field.child_by_field_name("pattern").is_some()
            || children(*field).iter().any(|part| matches!(part.kind(), "ref" | "mutable_specifier"))
            || !field.child_by_field_name("name").is_some_and(|name| name.kind() == "shorthand_field_identifier")
    }) { return None; }
    let arm = std::iter::successors(node.parent(), |parent| parent.parent())
        .take_while(|parent| !matches!(parent.kind(), "function_item" | "closure_expression"))
        .find(|parent| parent.kind() == "match_arm")?;
    let enclosing = arm.parent()?.parent()?;
    if enclosing.kind() != "match_expression" { return None; }
    let brace = children(node).into_iter().find(|part| part.kind() == "{" && !part.is_missing())?;
    let descriptions = fields.iter().filter_map(|field| field.child_by_field_name("name"))
        .map(|name| { let name = src.norm(name); format!("{name} selects the {name} field and binds it as {name} (shorthand for {name}: {name})") })
        .collect::<Vec<_>>().join("; ");
    let remaining = if named_children(node).iter().any(|part| part.kind() == "remaining_field_pattern") {
        " The .. ignores remaining fields."
    } else { "" };
    Some((enclosing, brace, ReadingContent::Detail {
        label: "Match and unpack named fields".into(),
        text: format!("{} identifies the struct or named-field variant to match; the declaration and name resolution determine which. Inside the braces, {}. If the pattern and any guard succeed, these names are available in this arm. Binding a field can move, copy, or borrow depending on its type and the matching context; this syntax alone does not establish which.{}", src.norm(owner), descriptions, remaining),
        definition: None,
        borrow: false,
    }))
}
