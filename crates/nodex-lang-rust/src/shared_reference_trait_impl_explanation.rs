//! Explain trait implementations on shared references to a generic type.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, module: Node<'_>) -> Vec<ContractItem> {
    let container = match module.kind() {
        "source_file" => Some(module),
        "mod_item" => module.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    named_children(container).into_iter().filter_map(|item| {
        if item.kind() != "impl_item" || item.has_error() || children(item).iter().any(|part| part.kind() == "!") { return None; }
        let implemented_trait = item.child_by_field_name("trait")?;
        let target = item.child_by_field_name("type")?;
        if target.kind() != "reference_type" || children(target).iter().any(|part| part.kind() == "mutable_specifier") { return None; }
        let referent = target.child_by_field_name("type")?;
        if referent.kind() != "type_identifier" { return None; }
        let parameters = item.child_by_field_name("type_parameters")?;
        let param = named_children(parameters).into_iter().find(|part| part.kind() == "type_parameter" && part.child_by_field_name("name").is_some_and(|name| src.norm(name) == src.norm(referent)))?;
        let _ = param;
        Some(ContractItem {
            group: "Trait implementation for a shared reference".into(),
            text: format!("This `impl` provides the trait `{}` for the shared-reference type `{}`, for each choice of the generic type `{}` that satisfies the declared bounds and where-clause requirements. `&` means the implementation is for a shared reference to the value, rather than necessarily for the owned value itself; it does not create a reference or call a method by itself. References must remain valid for their lifetime, and shared references do not ordinarily permit mutation through them (interior-mutability types are an exception). The trait and type names still need resolution, and Rust checks coherence and overlapping implementations.", src.norm(implemented_trait), src.norm(target), src.norm(referent)),
            span: src.header_span(item, item.child_by_field_name("body")), type_info: None,
        })
    }).collect()
}
