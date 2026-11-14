//! Beginner-facing explanation of Rust's shared-storage union declarations.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "union_item" { return None; }
    let name = node.child_by_field_name("name")?;
    let body = node.child_by_field_name("body")?;
    let fields = named_children(body).into_iter()
        .filter(|field| field.kind() == "field_declaration")
        .filter_map(|field| Some((field.child_by_field_name("name")?, field.child_by_field_name("type")?)))
        .map(|(name, ty)| format!("{}: {}", src.norm(name), src.norm(ty)))
        .collect::<Vec<_>>();
    let field_text = if fields.is_empty() { String::new() } else {
        format!(" Its alternative fields are {}: each colon gives a field's type.", fields.join(", "))
    };
    Some(ContractItem {
        group: "Union".into(),
        text: format!("union defines {} as a type whose fields share storage. Their memory can overlap, so writing one field can change bytes seen through another.{} The union does not track which field was last written. Reading a field requires unsafe and the programmer must ensure the stored bytes form a valid value of that field's type. This is shared storage, not a tagged choice like a Rust enum.", src.norm(name), field_text),
        span: src.span(node),
        type_info: None,
    })
}
