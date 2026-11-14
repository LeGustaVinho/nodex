//! Explain an explicitly written primitive type argument in a function call.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Highlight the argument list, leaving individual arguments free for other details.
pub(crate) fn explain<'a>(src: Src<'_>, node: Node<'a>) -> Option<(Node<'a>, ReadingContent)> {
    if node.kind() != "generic_function" { return None; }
    let call = node.parent().filter(|parent| parent.kind() == "call_expression")?;
    if call.child_by_field_name("function") != Some(node) { return None; }
    let function = node.child_by_field_name("function")?;
    if function.kind() != "identifier" { return None; }
    let arguments = node.child_by_field_name("type_arguments")?;
    let items = named_children(arguments);
    if items.len() != 1 || items[0].kind() != "primitive_type" { return None; }
    let callee = src.norm(function);
    let ty = src.norm(items[0]);
    Some((arguments, ReadingContent::Detail {
        label: "Explicit generic type argument".into(),
        text: format!("In {callee}::<{ty}>, ::<...> supplies a generic argument explicitly; it is often called the turbofish. Here {ty} is written as the type argument for {callee}, rather than asking the compiler to infer it. The following parentheses contain the ordinary value arguments passed to the call. Rust checks the generic argument against the function's generic parameters and their constraints; name resolution determines the actual function and type."),
        definition: None,
        borrow: false,
    }))
}
