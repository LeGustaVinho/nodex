//! Explain explicit closure parameter annotations without inferring capture or result types.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "closure_expression" || node.has_error() { return Vec::new(); }
    let Some(parameters) = node.child_by_field_name("parameters") else { return Vec::new(); };
    named_children(parameters).into_iter().filter_map(|parameter| {
        if parameter.kind() != "parameter" { return None; }
        let pattern = parameter.child_by_field_name("pattern")?;
        let ty = parameter.child_by_field_name("type")?;
        let colon = children(parameter).into_iter().find(|token| token.kind() == ":")?;
        Some((colon, ReadingContent::Detail {
            label: "Typed closure parameter".into(),
            text: format!("The vertical bars enclose the closure's parameters. In `{}: {}`, the colon declares the type expected for `{}` when the closure is called; the annotation is checked by Rust and is not a cast. The expression following the closing bar is the closure body and is evaluated when the closure is called, not when the closure is defined. If the body is an expression that completes, its value is the result of that call. An annotation may also be omitted when Rust can infer the type.", src.norm(pattern), src.norm(ty), src.norm(pattern)),
            definition: None,
            borrow: false,
        }))
    }).collect()
}
