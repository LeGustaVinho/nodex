//! Explain an empty braced struct expression without guessing its resolution.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "struct_expression" || node.has_error() { return None; }
    for ancestor in std::iter::successors(node.parent(), Node::parent) {
        if ancestor.kind() == "assignment_expression" {
            if ancestor.child_by_field_name("left").is_some_and(|left| left.start_byte() <= node.start_byte() && node.end_byte() <= left.end_byte()) { return None; }
            break;
        }
        if matches!(ancestor.kind(), "block" | "function_item") { break; }
    }
    let name = node.child_by_field_name("name")?;
    let body = node.child_by_field_name("body")?;
    if named_children(body).into_iter().any(|part| !part.is_extra()) { return None; }
    Some((name, ReadingContent::Detail {
        label: "Empty braced value construction".into(),
        text: format!("`{} {{}}` uses empty braces to construct a value with no fields supplied. If `{}` resolves to a unit-like struct declared as `struct {} ;`, this creates a value of that distinct zero-field type; the bare name can also construct it. The same braced syntax can refer to other suitable zero-field items, such as an empty named-field struct or variant, so the declaration and name resolution determine the actual type. The braces do not contain a block of executable statements, and syntax alone does not prove that this construction is valid or accessible.", src.norm(name), src.norm(name), src.norm(name)),
        definition: None, borrow: false,
    }))
}
