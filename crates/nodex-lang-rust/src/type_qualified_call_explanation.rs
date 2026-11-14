//! Explain a call through an explicitly bracketed type path such as `<T>::method()`.
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// The selected item still depends on type checking and name resolution.
pub(crate) fn explain<'t>(src: Src<'_>, call: Node<'t>) -> Option<(Node<'t>, String)> {
    if call.kind() != "call_expression" { return None; }
    let function = call.child_by_field_name("function")?;
    if function.kind() != "scoped_identifier" { return None; }
    let path = function.child_by_field_name("path")?;
    if path.kind() != "bracketed_type" || path.has_error() { return None; }
    let ty = named_children(path).into_iter().next()?;
    if ty.kind() == "qualified_type" { return None; }
    let method = function.child_by_field_name("name")?;
    Some((path, format!(
        "In {}, the angle brackets make {} an explicit type in the path. The following :: selects the associated item {} through that type; the parentheses call it. No trait is named here: the compiler resolves which applicable item is meant from the type and context, and an ambiguous trait method may require a fully qualified path such as <Type as Trait>::method(). This is a type-qualified call, not a call on a value named {}.",
        src.norm(function), src.norm(ty), src.norm(method), src.norm(ty)
    )))
}
