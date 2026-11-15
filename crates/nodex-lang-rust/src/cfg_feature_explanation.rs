//! Explain a simple outer `#[cfg(feature = "...")]` condition.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::{kit::truncate, syntax::{children, named_children, Src}};
use tree_sitter::Node;

fn feature_predicate<'t>(src: Src<'_>, item: Node<'t>) -> Option<(Node<'t>, Node<'t>)> {
    if item.kind() != "attribute_item" || item.has_error() { return None; }
    let attr = named_children(item).into_iter().find(|part| part.kind() == "attribute")?;
    let name = named_children(attr).into_iter().find(|part| part.kind() == "identifier")?;
    if src.text(name) != "cfg" { return None; }
    let args = attr.child_by_field_name("arguments")?;
    if args.kind() != "token_tree" || args.has_error() { return None; }
    let tokens: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
    let (feature, equal, value) = match tokens.as_slice() {
        [open, feature, equal, value, close] if open.kind() == "(" && close.kind() == ")" => (*feature, *equal, *value),
        [open, feature, equal, value, comma, close] if open.kind() == "(" && comma.kind() == "," && close.kind() == ")" => (*feature, *equal, *value),
        _ => return None,
    };
    if feature.kind() != "identifier" || src.text(feature) != "feature" || equal.kind() != "=" || value.kind() != "string_literal" || value.has_error() { return None; }
    Some((feature, value))
}

/// Explain the declaration's own outer cfg attributes and an enclosing impl's
/// attributes when inspecting one of its members. Do not evaluate cfg here.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "enum_variant" | "union_item" | "trait_item" | "type_item" | "associated_type" | "const_item" | "static_item" | "field_declaration" | "mod_item" | "macro_definition" | "impl_item") { return Vec::new(); }
    let owner = node.parent().and_then(|body| body.parent()).filter(|parent| parent.kind() == "impl_item");
    let mut result = Vec::new();
    for declaration in std::iter::once(node).chain(owner) {
        let mut previous = declaration.prev_named_sibling();
        while let Some(part) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
            if let Some((feature, value)) = feature_predicate(src, part) {
                let span = src.index.span(src.file, feature.start_byte(), value.end_byte());
                result.push(ContractItem {
                    group: "Feature-gated compilation".into(),
                    text: format!("This outer #[cfg(...)] condition checks whether the active compilation configuration contains the key-value option feature = {}. If it is true, this attribute allows the attached item to remain; other conditions can still exclude it. If false, conditional compilation removes the item before ordinary runtime execution. This is a build-time selection, not a runtime if. Cargo commonly supplies such feature options for the package being built when features are enabled, but compiler flags and the build setup determine the active configuration. The written attribute alone does not establish whether this feature is enabled in the current build.", truncate(src.text(value), 100)),
                    span, type_info: None,
                });
            }
            previous = part.prev_named_sibling();
        }
    }
    result
}
