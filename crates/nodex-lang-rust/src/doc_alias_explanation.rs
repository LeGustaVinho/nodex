//! Explain `#[doc(alias = "...")]` search aliases without treating them as Rust names.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

/// Extract direct `alias = <string>` entries only. Nested `alias(...)`,
/// `cfg_attr`, expression-valued aliases and inner attributes are distinct forms.
fn alias_values<'t>(src: Src<'_>, item: Node<'t>) -> Vec<Node<'t>> {
    if item.kind() != "attribute_item" || item.has_error() { return Vec::new(); }
    let Some(attr) = named_children(item).into_iter().find(|part| part.kind() == "attribute") else { return Vec::new(); };
    let Some(name) = named_children(attr).into_iter().find(|part| part.kind() == "identifier") else { return Vec::new(); };
    if src.text(name) != "doc" { return Vec::new(); }
    let Some(args) = attr.child_by_field_name("arguments") else { return Vec::new(); };
    if args.kind() != "token_tree" || args.has_error() { return Vec::new(); }
    let parts: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
    if parts.len() < 3 || parts.first().is_none_or(|part| part.kind() != "(") || parts.last().is_none_or(|part| part.kind() != ")") { return Vec::new(); }
    parts[1..parts.len() - 1].split(|part| part.kind() == ",").filter_map(|entry| match entry {
        [key, equal, value] if key.kind() == "identifier" && src.text(*key) == "alias" && equal.kind() == "="
            && matches!(value.kind(), "string_literal" | "raw_string_literal") && !value.has_error() => Some(*value),
        _ => None,
    }).collect()
}

/// Explain aliases on the attached symbol, without deciding whether rustdoc is
/// run or whether its output includes the item.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "enum_variant" | "union_item" | "trait_item" | "type_item" | "associated_type" | "const_item" | "static_item" | "field_declaration" | "mod_item" | "macro_definition" | "impl_item") { return Vec::new(); }
    let mut result = Vec::new();
    let mut previous = node.prev_named_sibling();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        for value in alias_values(src, attribute) {
            result.push(ContractItem {
                group: "Documentation search alias".into(),
                text: format!("This #[doc(alias = {})] attribute gives the attached item an additional name for rustdoc's documentation search index. The alias is the value of the Rust string literal written here, interpreted using Rust's string-literal rules. It helps readers find the documented item; it does not rename the item, create a Rust identifier or path, or execute code at runtime. Whether the item appears in generated documentation depends on the documentation build and its visibility settings; this attribute alone does not prove that rustdoc has been run.", src.text(value)),
                span: src.span(value), type_info: None,
            });
        }
        previous = attribute.prev_named_sibling();
    }
    result
}
