//! Explain explicit generic arguments on the type side of an associated path.
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

/// Return the written arguments, preserving name-resolution uncertainty.
pub(crate) fn explain<'t>(src: Src<'_>, call: Node<'t>) -> Option<(Node<'t>, String)> {
    if call.kind() != "call_expression" { return None; }
    let function = call.child_by_field_name("function")?;
    if function.kind() != "scoped_identifier" { return None; }
    let path = function.child_by_field_name("path")?;
    if path.kind() != "generic_type" { return None; }
    let name = path.child_by_field_name("type")?;
    let arguments = path.child_by_field_name("type_arguments")?;
    let method = function.child_by_field_name("name")?;
    // Restrict this explanation to an explicitly written turbofish.
    if !src.text(path).contains("::") { return None; }
    Some((arguments, format!(
        "In {}, the :: before {} supplies explicit generic arguments to {}. The following :: selects {} through that instantiated path; the parentheses then call the selected item. The arguments belong to the type-side path, not necessarily to {} itself. Which item is selected and whether the arguments are valid depend on name resolution and Rust's type checking.",
        src.norm(function), src.norm(arguments), src.norm(name), src.norm(method), src.norm(method)
    )))
}
