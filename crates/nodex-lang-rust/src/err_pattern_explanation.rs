//! Explain `Err(inner)` patterns without assuming which `Err` was resolved.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "tuple_struct_pattern" || node.has_error() { return None; }
    let path = node.child_by_field_name("type")?;
    let name = if path.kind() == "scoped_identifier" { path.child_by_field_name("name")? } else { path };
    if src.norm(name) != "Err" { return None; }
    let inner: Vec<_> = named_children(node).into_iter().filter(|child| *child != path).collect();
    // `_` is an unnamed child and is itself a valid inner pattern.
    let wildcard = children(node).into_iter().find(|child| child.kind() == "_");
    if inner.len() + usize::from(wildcard.is_some()) != 1 { return None; }
    let payload = inner.first().copied().or(wildcard)?;
    let mut anchor = name;
    let mut parent = node.parent();
    while let Some(current) = parent {
        if current.kind() == "match_arm" {
            if let Some(owner) = current.parent().and_then(|body| body.parent()) {
                if let Some(keyword) = children(owner).into_iter().find(|child| child.kind() == "match") { anchor = keyword; }
            }
            break;
        }
        if matches!(current.kind(), "let_condition" | "let_declaration" | "for_expression" | "parameter" | "closure_parameters") { break; }
        parent = current.parent();
    }
    Some((anchor, name, ReadingContent::Detail {
        label: "Match an Err-shaped value".into(),
        text: format!("This pattern checks whether the value has the {} variant or constructor and applies the inner pattern {} to its contained part. If Err resolves to the standard Result::Err variant, it represents the error case of a Result, and its contained part is the error value. The spelling alone does not prove that resolution: another type may define Err. The inner pattern may bind, ignore, or further test that part; a binding moves, copies, or borrows according to its pattern and type. Matching does not itself throw or return the error.", src.norm(path), src.norm(payload)),
        definition: None, borrow: false,
    }))
}
