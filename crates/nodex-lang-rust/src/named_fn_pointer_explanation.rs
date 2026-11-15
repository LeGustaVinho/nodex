//! Explain parameter names inside function-pointer types on local bindings.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    if ty.kind() != "function_type" || ty.child_by_field_name("trait").is_some() { return Vec::new(); }
    let Some(params) = ty.child_by_field_name("parameters") else { return Vec::new(); };
    named_children(params).into_iter().filter_map(|param| {
        if param.kind() != "parameter" { return None; }
        let name = param.child_by_field_name("pattern")?;
        if name.kind() != "identifier" { return None; }
        let ty = param.child_by_field_name("type")?;
        Some((name, ReadingContent::Detail {
            label: "Named function pointer parameter".into(),
            text: format!("In this function-pointer type, `{}: {}` describes an input of type `{}` and gives that input the documentation name `{}`. The name is optional and does not change the function pointer's type, declare a local variable, or create named arguments at a call site: calls still pass arguments by position. The value assigned to this variable must be compatible with the pointer's full signature, including types and calling convention; the source spelling alone does not resolve or validate the assigned function.", src.norm(name), src.norm(ty), src.norm(ty), src.norm(name)),
            definition: None, borrow: false,
        }))
    }).collect()
}
