//! Explain free-standing type aliases without confusing associated type definitions.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "type_item" { return None; }
    let parent = node.parent()?;
    if parent.kind() != "source_file"
        && !(parent.kind() == "declaration_list" && parent.parent().is_some_and(|p| p.kind() == "mod_item")) {
        return None;
    }
    let name = node.child_by_field_name("name")?;
    let target = node.child_by_field_name("type")?;
    let name = src.norm(name);
    let target = src.norm(target);
    let example = if target == "Vec<u8>" {
        " If Vec refers to Rust's standard Vec, Vec<u8> is a growable sequence of unsigned 8-bit integers; the angle brackets provide its element type."
    } else { "" };
    Some(ContractItem {
        group: "Type alias".into(),
        text: format!("type {name} = {target} gives the existing type {target} another name, {name}. The equals sign defines the alias; it does not create a distinct type or a new value. Code can use the alias where the original type is expected.{example}"),
        span: src.span(node),
        type_info: None,
    })
}
