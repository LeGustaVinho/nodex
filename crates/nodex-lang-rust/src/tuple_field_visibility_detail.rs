//! Explain `pub(crate)` on an individual positional struct field.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "struct_item" || node.has_error() { return Vec::new(); }
    let Some(body) = node.child_by_field_name("body").filter(|n| n.kind() == "ordered_field_declaration_list") else { return Vec::new(); };
    let Some(name) = node.child_by_field_name("name") else { return Vec::new(); };
    let mut visibility = None;
    let mut index = 0usize;
    let mut result = Vec::new();
    for child in named_children(body) {
        if child.is_extra() || child.kind() == "attribute_item" { continue; }
        if child.kind() == "visibility_modifier" { visibility = Some(child); continue; }
        if let Some(modifier) = visibility.take().filter(|v| src.norm(*v).replace(' ', "") == "pub(crate)") {
            result.push(ContractItem {
                group: "Crate-visible tuple struct field".into(),
                text: format!("In tuple struct {}, `pub(crate)` applies to the positional field written at index {} with type {}. It allows access from within the current crate where the struct itself is accessible, but not directly from another crate; the struct's own visibility can be more restrictive. A tuple struct's constructor is also limited by the visibility of its fields and the struct, so this field alone does not make the type or constructor public outside the crate. Fields are accessed by numeric position, such as `.0`. Conditional compilation can change which fields exist and their positions.", src.norm(name), index, src.norm(child)),
                span: src.span(modifier), type_info: None,
            });
        }
        index += 1;
    }
    result
}
