//! Explain the basic pipe-delimited closure syntax.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

/// Explain synchronous closures with a single untyped name and expression body.
/// Other forms (patterns, typed parameters, blocks, async) need their own details.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "closure_expression" || node.has_error() || children(node).iter().any(|child| child.kind() == "async") { return None; }
    let params = node.child_by_field_name("parameters")?;
    let names = named_children(params);
    if names.len() != 1 || names[0].kind() != "identifier" { return None; }
    let body = node.child_by_field_name("body")?;
    if body.kind() == "block" { return None; }
    let pipe = children(params).into_iter().find(|token| token.kind() == "|" && !token.is_missing())?;
    let name = src.norm(names[0]);
    Some((pipe, ReadingContent::Detail {
        label: "Closure parameters and result".into(),
        text: format!("The vertical bars surround the parameter list: {name} names the argument supplied when this anonymous function value is called. Creating the closure does not run its body; each call evaluates the expression after the closing bar, and that expression's value becomes the call's result. The compiler can infer omitted parameter and result types from the surrounding context and uses. The closure may also capture values from its surrounding scope; their capture mode depends on how they are used and on move."),
        definition: None,
        borrow: false,
    }))
}
