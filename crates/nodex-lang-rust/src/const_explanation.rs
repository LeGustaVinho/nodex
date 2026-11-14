use nodex_ir::{Language, reading::ContractItem, types::{TypeInfo, TypeOrigin, syntax_shape}};
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

/// Explain a constant declaration without assuming the reader knows Rust syntax.
pub(crate) fn explain(src: Src, node: Node) -> Option<ContractItem> {
    if node.kind() != "const_item" { return None; }
    let name = node.child_by_field_name("name")?;
    let ty = node.child_by_field_name("type")?;
    let value = node.child_by_field_name("value")?;
    let native = src.norm(ty);
    Some(ContractItem {
        group: "Constant".into(),
        text: format!(
            "{} names a constant of type {}. Its initializer {} is evaluated at compile time; its value cannot be reassigned. Each use refers to the constant value, not necessarily one shared memory location.",
            src.norm(name), native, src.norm(value),
        ),
        span: src.span(node),
        type_info: Some(TypeInfo::new(syntax_shape(Language::Rust, &native), TypeOrigin::Declared, Some(native))),
    })
}
