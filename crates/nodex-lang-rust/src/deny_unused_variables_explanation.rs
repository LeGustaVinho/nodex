//! Explain the built-in `unused_variables` lint at deny level.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

/// Identify a direct, unqualified `unused_variables` entry in `#[deny(...)]`.
/// Splitting only at this token tree's top-level commas prevents matching a
/// different tool lint, a string in `reason = ...`, or nested cfg_attr tokens.
fn denied_unused_variables<'t>(src: Src<'_>, item: Node<'t>) -> Option<Node<'t>> {
    if item.kind() != "attribute_item" || item.has_error() { return None; }
    let attr = named_children(item).into_iter().find(|part| part.kind() == "attribute")?;
    let name = named_children(attr).into_iter().find(|part| part.kind() == "identifier")?;
    if src.text(name) != "deny" { return None; }
    let args = attr.child_by_field_name("arguments")?;
    if args.kind() != "token_tree" || args.has_error() { return None; }
    let parts: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
    if parts.len() < 3 || parts.first()?.kind() != "(" || parts.last()?.kind() != ")" { return None; }
    let mut entry: Vec<Node<'t>> = Vec::new();
    for part in &parts[1..parts.len() - 1] {
        if part.kind() == "," {
            if let [candidate] = entry.as_slice() {
                if candidate.kind() == "identifier" && src.text(*candidate) == "unused_variables" { return Some(*candidate); }
            }
            entry.clear();
        } else { entry.push(*part); }
    }
    if let [candidate] = entry.as_slice() {
        if candidate.kind() == "identifier" && src.text(*candidate) == "unused_variables" { return Some(*candidate); }
    }
    None
}

/// Surface the written outer attribute on its declaration, and on a method
/// whose enclosing impl has this lint level. This does not run lint analysis.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "enum_variant" | "union_item" | "trait_item" | "type_item" | "associated_type" | "const_item" | "static_item" | "field_declaration" | "mod_item" | "macro_definition" | "impl_item") { return Vec::new(); }
    let owner = node.parent().and_then(|body| body.parent()).filter(|parent| parent.kind() == "impl_item");
    let mut result = Vec::new();
    for declaration in std::iter::once(node).chain(owner) {
        let mut previous = declaration.prev_named_sibling();
        while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
            if let Some(lint) = denied_unused_variables(src, attribute) {
                result.push(ContractItem {
                    group: "Deny unused-variable lint".into(),
                    text: "This outer #[deny(unused_variables)] attribute sets the unused_variables lint to deny for the attached item and its scope. That lint checks for variable bindings which are not used. If rustc reports it at the effective deny level, the diagnostic is an error and normally prevents successful compilation rather than being only a warning. Inner lint attributes, command-line settings, and lint caps can affect the effective level. The attribute alone does not prove any variable is unused or that this build fails. It controls compile-time diagnostics; it is not a runtime instruction.".into(),
                    span: src.span(lint), type_info: None,
                });
            }
            previous = attribute.prev_named_sibling();
        }
    }
    result
}
