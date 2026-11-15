//! Recover simple numeric field patterns even when tree-sitter marks them as errors.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "struct_pattern" { return None; }
    let name = node.child_by_field_name("type")?;
    let parts: Vec<_> = children(node).into_iter().filter(|part| !part.is_extra() || part.kind() == "ERROR").collect();
    let open = parts.iter().position(|part| part.kind() == "{")?;
    let close = parts.iter().rposition(|part| part.kind() == "}")?;
    if open >= close { return None; }
    let mut descriptions = Vec::new();
    for segment in parts[open+1..close].split(|part| part.kind() == ",") {
        if segment.is_empty() { continue; }
        let [error, field] = segment else { return None; };
        if error.kind() != "ERROR" || field.kind() != "field_pattern" { return None; }
        let error_parts = children(*error);
        let [index, colon] = error_parts.as_slice() else { return None; };
        if index.kind() != "integer_literal" || colon.kind() != ":" || !src.text(*index).chars().all(|ch| ch.is_ascii_digit()) { return None; }
        let binding = named_children(*field);
        let [binding] = binding.as_slice() else { return None; };
        if binding.kind() != "shorthand_field_identifier" { return None; }
        descriptions.push(format!("field {} binds {}", src.text(*index), src.text(*binding)));
    }
    if descriptions.is_empty() { return None; }
    Some((parts[open], ReadingContent::Detail {
        label: "Match numeric fields in braces".into(),
        text: format!("In the pattern `{}`, braces select fields by their zero-based numeric position: {}. If `{}` resolves to a tuple struct or tuple-like variant, these entries match its positional fields; the numbers are field labels, not array indexing. The names introduce bindings when the pattern matches; whether a field is moved, copied or borrowed depends on its type and binding context. In a plain `let`, the pattern must be irrefutable. The written path alone does not establish its resolution or whether the pattern is valid.", src.norm(node), descriptions.join("; "), src.norm(name)),
        definition: None, borrow: false,
    }))
}
