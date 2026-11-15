//! Explain the relationship between a tuple parameter's pattern and its type.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let Some(parameters) = node.child_by_field_name("parameters") else { return Vec::new(); };
    named_children(parameters).into_iter().filter_map(|parameter| {
        let pattern = parameter.child_by_field_name("pattern")?;
        let ty = parameter.child_by_field_name("type")?;
        if pattern.kind() != "tuple_pattern" || pattern.has_error() || !children(pattern).iter().any(|part| part.kind() == ",") { return None; }
        Some(ContractItem {
            group: "Tuple parameter".into(),
            text: format!("{}: {} is one parameter, whose argument must be a tuple matching the declared type. When the function is called, {} destructures that one tuple: each position binds or matches the corresponding element in order, making its bindings available inside the function. For example, (a, b): (i32, i32) takes one pair of 32-bit signed integers, binding its first element to a and its second to b. Binding by value copies or moves according to the element's type; patterns can also borrow or ignore elements. Parameter patterns must be irrefutable, so they must match every possible value of the parameter type.", src.norm(pattern), src.norm(ty), src.norm(pattern)),
            span: src.span(parameter), type_info: None,
        })
    }).collect()
}
