//! Explain a return-position `impl Trait + '_` tied to one `&str` input.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") || node.has_error() { return None; }
    let params = node.child_by_field_name("parameters")?;
    let params: Vec<_> = named_children(params).into_iter().filter(|part| !part.is_extra()).collect();
    let [parameter] = params.as_slice() else { return None; };
    if parameter.kind() != "parameter" { return None; }
    let input = parameter.child_by_field_name("type")?;
    if input.kind() != "reference_type" { return None; }
    let target = input.child_by_field_name("type")?;
    if target.kind() != "primitive_type" || src.text(target) != "str" { return None; }
    let parameter_name = parameter.child_by_field_name("pattern")?;
    let result = node.child_by_field_name("return_type")?;
    if result.kind() != "bounded_type" { return None; }
    let result_parts: Vec<_> = named_children(result).into_iter().filter(|part| !part.is_extra()).collect();
    let [opaque, lifetime] = result_parts.as_slice() else { return None; };
    if opaque.kind() != "abstract_type" || lifetime.kind() != "lifetime" || src.norm(*lifetime) != "'_" { return None; }
    let bound = opaque.child_by_field_name("trait")?;
    Some(ContractItem {
        group: "Inferred lifetime bound on an opaque return".into(),
        text: format!("The function returns some concrete type chosen by its implementation, exposed through the {} trait bound rather than by naming that concrete type. In this signature the input {} is the only reference parameter, so Rust's lifetime-elision rules infer '_ from that input reference's lifetime. The + '_ is an outlives bound: the hidden return type must remain valid for that inferred lifetime, including any references it contains. It does not prove that the return actually borrows {}, keep that value alive, or extend a borrow. The body must still satisfy the declared bound; exact generic capture rules depend on the language edition and any explicit capture bounds. This syntax alone does not imply heap allocation or dynamic dispatch.", src.norm(bound), src.norm(parameter_name), src.norm(parameter_name)),
        span: src.span(*lifetime), type_info: None,
    })
}
