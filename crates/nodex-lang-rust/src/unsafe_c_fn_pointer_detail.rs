//! Connect an unsafe C function pointer annotation to its initializer.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" || node.has_error() { return None; }
    let ty = node.child_by_field_name("type")?;
    if ty.kind() != "function_type" || ty.child_by_field_name("trait").is_some() { return None; }
    let modifiers = named_children(ty).into_iter().find(|part| part.kind() == "function_modifiers")?;
    if !children(modifiers).iter().any(|part| part.kind() == "unsafe") { return None; }
    let external = named_children(modifiers).into_iter().find(|part| part.kind() == "extern_modifier")?;
    let abi = named_children(external).into_iter().find(|part| part.kind() == "string_literal")?;
    if src.text(abi) != "\"C\"" { return None; }
    let binding = node.child_by_field_name("pattern")?;
    let value = node.child_by_field_name("value")?;
    let params = ty.child_by_field_name("parameters")?;
    let result = ty.child_by_field_name("return_type").map(|part| src.norm(part)).unwrap_or_else(|| "()".into());
    Some((ty, ReadingContent::Detail {
        label: "Bind an unsafe C function pointer".into(),
        text: format!("This `let` binds {} to the value of {}, with the function pointer type `{}`. A compatible function value can be stored and later called with arguments {} and result type {}; the initializer does not call it. `extern \"C\"` specifies the target's C calling convention. `unsafe` means a call through this pointer requires an unsafe context and the caller must uphold the pointed-to function's safety requirements. The source alone does not resolve the initializer, establish signature compatibility or reveal those requirements.", src.norm(binding), src.norm(value), src.norm(ty), src.norm(params), result),
        definition: None, borrow: false,
    }))
}
