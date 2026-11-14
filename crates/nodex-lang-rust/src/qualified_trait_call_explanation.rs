//! Explain fully qualified trait-associated calls without resolving their names.
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, call: Node<'t>) -> Option<(Node<'t>, String)> {
    if call.kind() != "call_expression" { return None; }
    let function = call.child_by_field_name("function")?;
    if function.kind() != "scoped_identifier" { return None; }
    let path = function.child_by_field_name("path")?;
    if path.kind() != "bracketed_type" { return None; }
    let qualified = named_children(path).into_iter().find(|n| n.kind() == "qualified_type")?;
    let target = qualified.child_by_field_name("type")?;
    let tr = qualified.child_by_field_name("alias")?;
    let member = function.child_by_field_name("name")?;
    Some((qualified, format!(
        "In {}, the part <{} as {}> selects the implementation of trait {} for type {}. The following ::{} names an associated item through that implementation; the parentheses call it. If {} is an instance method, its receiver is supplied explicitly among the call arguments. This spelling disambiguates the trait path, but the compiler must still resolve the names and verify that the implementation and call are valid.",
        src.norm(function), src.norm(target), src.norm(tr), src.norm(tr), src.norm(target), src.norm(member), src.norm(member),
    )))
}
