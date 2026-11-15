//! Explain a named shared-reference field binding combined with omitted fields.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "struct_pattern" || node.has_error() { return Vec::new(); }
    if !named_children(node).iter().any(|part| part.kind() == "remaining_field_pattern") { return Vec::new(); }
    let mut result = Vec::new();
    for field in named_children(node).into_iter().filter(|part| part.kind() == "field_pattern") {
        let Some(name) = field.child_by_field_name("name") else { continue; };
        let Some(reference) = named_children(field).into_iter().find(|part| part.kind() == "ref_pattern") else { continue; };
        if children(reference).iter().any(|part| matches!(part.kind(), "mut_pattern" | "mutable_specifier")) { continue; }
        let Some(binding) = named_children(reference).into_iter().find(|part| part.kind() == "identifier") else { continue; };
        result.push((reference, ReadingContent::Detail {
            label: "Borrow a field and omit the rest".into(),
            text: format!("In this pattern, `{}: ref {}` selects field {} and binds {} to a shared reference to that field; it does not move the field into {}. The `..` allows all other fields to be omitted and leaves them unbound by this pattern. This is matching an existing value, not constructing one. The borrow must remain valid while used, and the pattern must be valid for the matched value and its context; the type or variant named by the pattern depends on name resolution.", src.norm(name), src.norm(binding), src.norm(name), src.norm(binding), src.norm(binding)),
            definition: None, borrow: false,
        }));
    }
    result
}
