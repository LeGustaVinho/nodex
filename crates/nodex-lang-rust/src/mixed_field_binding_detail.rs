//! Explain mixed shorthand `ref` and `mut` field bindings.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "struct_pattern" || node.has_error() { return None; }
    let fields: Vec<_> = named_children(node).into_iter().filter(|part| part.kind() == "field_pattern").collect();
    let mut borrowed = None;
    let mut mutable = None;
    for field in fields {
        let Some(name) = field.child_by_field_name("name").filter(|part| part.kind() == "shorthand_field_identifier") else { continue; };
        let parts = children(field);
        let has_ref = parts.iter().any(|part| part.kind() == "ref");
        let has_mut = parts.iter().any(|part| part.kind() == "mutable_specifier");
        if has_ref && !has_mut { borrowed = Some(name); }
        if has_mut && !has_ref { mutable = Some(name); }
    }
    let borrowed = borrowed?;
    let mutable = mutable?;
    Some((node, ReadingContent::Detail {
        label: "Mixed field binding modes".into(),
        text: format!("In `{}`, shorthand `ref {}` selects the field of the same name and binds {} to a shared reference to it, without moving that field. Shorthand `mut {}` selects its namesake field and creates a mutable local binding {} that may be reassigned; `mut` alone does not create a mutable reference to the original field. The latter field is copied or moved according to its type and binding mode. These are separate bindings in a pattern that matches an existing value; name resolution and type checking determine whether the pattern is valid.", src.norm(node), src.norm(borrowed), src.norm(borrowed), src.norm(mutable), src.norm(mutable)),
        definition: None, borrow: false,
    }))
}
