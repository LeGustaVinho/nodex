//! Explain a literal constant argument written in a turbofish.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Only recognize a numeric literal directly in an explicit generic argument list.
/// The syntax does not establish which declaration the path resolves to.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "generic_function" { return Vec::new(); }
    let Some(args) = node.child_by_field_name("type_arguments") else { return Vec::new() };
    named_children(args).into_iter().filter(|arg| arg.kind() == "integer_literal")
        .map(|arg| (arg, ReadingContent::Detail {
            label: "Explicit constant generic argument".into(),
            text: format!("The ::<...> syntax supplies generic arguments explicitly. Here {} is written as a constant argument, not as a run-time argument between the call's parentheses. If the selected item has a compatible const generic parameter at this position, Rust uses that value when instantiating it and checks its type and validity at compile time. The path must resolve to such an item; this syntax alone does not identify the declaration.", src.norm(arg)),
            definition: None,
            borrow: false,
        })).collect()
}
