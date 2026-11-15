//! Explain a struct declared with an empty named-field list.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "struct_item" || node.has_error() { return None; }
    let name = node.child_by_field_name("name")?;
    let body = node.child_by_field_name("body")?;
    if body.kind() != "field_declaration_list" || named_children(body).iter().any(|child| child.kind() == "field_declaration") { return None; }
    let written = src.norm(name);
    Some(ContractItem {
        group: "Empty braced struct".into(),
        text: format!("`struct {written} {{}}` declares a distinct named-field struct type with no fields. A value is constructed with empty braces, as `{written} {{}}`, where construction is permitted. This differs from a unit-like declaration `struct Marker;`: its bare name `Marker` can also be used as a value, whereas the bare name of this braced struct is a type, not its value constructor. The empty braces in the declaration list fields, not executable statements. Both forms define distinct Rust types; neither is the unit type `()`.") ,
        span: src.span(body), type_info: None,
    })
}
