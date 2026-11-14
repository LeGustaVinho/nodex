use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src, node: Node) -> Option<ContractItem> {
    if node.kind() != "struct_item" || node.child_by_field_name("body").is_some() { return None; }
    let name = node.child_by_field_name("name")?;
    let written = src.norm(name);
    Some(ContractItem {
        group: "Unit struct".into(),
        text: format!("{written} defines a distinct struct type with no fields. Writing {written} as a value constructs an instance without storing field data. Such types are often used as markers or to implement behavior without per-instance data."),
        span: src.span(node),
        type_info: None,
    })
}
