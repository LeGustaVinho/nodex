//! Explain the zero-field tuple-struct declaration specifically.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "struct_item" || node.has_error() { return None; }
    let body = node.child_by_field_name("body")?;
    if body.kind() != "ordered_field_declaration_list" || named_children(body).into_iter().any(|child| !child.is_extra()) { return None; }
    let name = src.norm(node.child_by_field_name("name")?);
    Some(ContractItem {
        group: "Empty tuple struct".into(),
        text: format!("`struct {name}();` declares a distinct tuple-struct type with zero fields. The generated constructor `{name}()` takes no arguments and creates a value of this type, when the constructor is accessible. It is a zero-sized type and has no `.0` field to access. Despite the parentheses, the value is not the unit value `()` and the type is not the unit type; `struct {name};` instead declares a different, unit-like struct form. The declaration itself does not call the constructor."),
        span: src.span(node), type_info: None,
    })
}
