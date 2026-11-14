//! Explain the `mut` modifier on a name bound by a pattern.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    let (name, keyword) = match node.kind() {
        "mut_pattern" if !node.parent().is_some_and(|parent| parent.kind() == "ref_pattern") => {
            let name = named_children(node).into_iter().find(|child| child.kind() == "identifier")?;
            let keyword = children(node).into_iter().find(|child| child.kind() == "mutable_specifier")?;
            (name, keyword)
        }
        "field_pattern" if !children(node).iter().any(|child| child.kind() == "ref") => {
            let name = node.child_by_field_name("name").filter(|name| name.kind() == "shorthand_field_identifier")?;
            let keyword = children(node).into_iter().find(|child| child.kind() == "mutable_specifier")?;
            (name, keyword)
        }
        _ => return None,
    };
    let name = src.norm(name);
    Some((keyword, ReadingContent::Detail {
        label: "Mutable pattern binding".into(),
        text: format!("The pattern creates a local binding named {name}. `mut` allows assigning a new value to this binding after it is initialized. It describes the binding, not a mutable borrow of the matched value: whether the matched part is moved, copied, or borrowed depends on the rest of the pattern and its type. Other names in the pattern are unaffected."),
        definition: None,
        borrow: false,
    }))
}
