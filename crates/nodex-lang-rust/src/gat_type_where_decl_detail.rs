//! Explain a type-generic associated type with a bound on its own parameter.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "associated_type" || node.has_error() { return Vec::new(); }
    let Some(owner) = node.parent().and_then(|body| body.parent()).filter(|part| part.kind() == "trait_item") else { return Vec::new(); };
    let (Some(name), Some(trait_name), Some(parameters)) = (node.child_by_field_name("name"), owner.child_by_field_name("name"), node.child_by_field_name("type_parameters")) else { return Vec::new() };
    let names: Vec<_> = named_children(parameters).into_iter().filter(|part| part.kind() == "type_parameter").filter_map(|part| part.child_by_field_name("name")).map(|part| src.norm(part)).collect();
    let Some(clause) = named_children(node).into_iter().find(|part| part.kind() == "where_clause") else { return Vec::new(); };
    let mut result = Vec::new();
    for predicate in named_children(clause).into_iter().filter(|part| part.kind() == "where_predicate") {
        let (Some(left), Some(bounds)) = (predicate.child_by_field_name("left"), predicate.child_by_field_name("bounds")) else { continue; };
        let parameter = src.norm(left);
        if !names.contains(&parameter) { continue; }
        let clone = named_children(bounds).into_iter().any(|part| src.norm(part) == "Clone");
        let extra = if clone { format!(" If `Clone` resolves to Rust's standard trait, the chosen {parameter} must implement it; the bound does not itself clone a value.") } else { String::new() };
        result.push(ContractItem {
            group: "Generic associated type parameter requirement".into(),
            text: format!("In trait {}, `type {}{}` declares a generic associated type: each implementation supplies a family of associated types rather than a runtime value. The `where {}` requirement applies to its own type parameter {parameter}; code selecting a member of this family must satisfy that requirement for its chosen argument.{extra} The trait name and bounds still require resolution and checking, and this declaration alone does not choose the implementation's resulting type.", src.norm(trait_name), src.norm(name), src.norm(parameters), src.norm(predicate)),
            span: src.span(predicate), type_info: None,
        });
    }
    result
}
