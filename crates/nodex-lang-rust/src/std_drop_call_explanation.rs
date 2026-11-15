//! Explain an explicit call to the standard library's drop function.
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, String)> {
    if node.kind() != "call_expression" || node.has_error() { return None; }
    let callee = node.child_by_field_name("function")?;
    let path: String = src.text(callee).chars().filter(|c| !c.is_whitespace()).collect();
    if !matches!(path.as_str(), "std::mem::drop" | "::std::mem::drop") { return None; }
    let arguments = node.child_by_field_name("arguments")?;
    let values = named_children(arguments);
    if values.len() != 1 { return None; }
    Some((callee, format!("If {} resolves to Rust's standard std::mem::drop function, it takes its argument by value and returns (). The passed value is dropped when the function ends, so owned resources can be released before the surrounding scope ends and its destructor runs if applicable. For a non-Copy value passed by value, ownership moves into the call and that value cannot subsequently be used unless reinitialized. For a Copy value, the call receives and drops a copy; the original remains usable. Dropping a reference drops only that reference, not the referenced value. The exact effects depend on the argument type; the written path alone does not establish name resolution or whether the argument is Copy.", src.norm(callee))))
}
