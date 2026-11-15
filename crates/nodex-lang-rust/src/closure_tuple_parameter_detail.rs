//! Explain a typed tuple pattern as one closure parameter.
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
        if pattern.kind() != "tuple_pattern" || ty.kind() != "tuple_type" { return None; }
        let bindings: Vec<_> = named_children(pattern).into_iter().filter(|part| !part.is_extra()).collect();
        let types: Vec<_> = named_children(ty).into_iter().filter(|part| !part.is_extra()).collect();
        if bindings.len() < 2 || bindings.len() != types.len() || !bindings.iter().all(|part| part.kind() == "identifier") { return None; }
        let open = children(pattern).into_iter().find(|part| part.kind() == "(")?;
        let mappings = bindings.iter().zip(types.iter()).enumerate().map(|(index, (binding, ty))| format!("element {index} binds `{}` with type `{}`", src.norm(*binding), src.norm(*ty))).collect::<Vec<_>>().join("; ");
        Some((open, ReadingContent::Detail {
            label: "Typed tuple closure parameter".into(),
            text: format!("`{}: {}` is one closure parameter, not {} separate arguments. When the closure is called, its corresponding argument must be a tuple of the annotated type, and the pattern splits that tuple by position: {}. The vertical bars surround the full parameter list. The body after the closing bar runs on each call, and its expression result becomes the call result when it completes. The annotation is a type requirement, not a cast; bindings by value can copy or move according to their types.", src.norm(pattern), src.norm(ty), bindings.len(), mappings),
            definition: None, borrow: false,
        }))
    }).collect()
}
