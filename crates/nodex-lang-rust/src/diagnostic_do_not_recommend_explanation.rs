//! Explain `#[diagnostic::do_not_recommend]` on a trait implementation.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn item(src: Src<'_>, implementation: Node<'_>) -> Vec<ContractItem> {
    if implementation.kind() != "impl_item" || implementation.has_error() || implementation.child_by_field_name("trait").is_none() || children(implementation).iter().any(|child| child.kind() == "!") { return Vec::new(); }
    let mut previous = implementation.prev_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" => {
                if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute") {
                    let path = named_children(attribute).into_iter().next();
                    if path.is_some_and(|path| src.norm(path) == "diagnostic::do_not_recommend") && attribute.child_by_field_name("arguments").is_none() && attribute.child_by_field_name("value").is_none() {
                        result.push(ContractItem {
                            group: "Diagnostic recommendation hint".into(),
                            text: "`#[diagnostic::do_not_recommend]` is a hint for diagnostics on this trait implementation: when explaining a trait error, the compiler is asked not to show this implementation as a recommendation. This can avoid a misleading suggestion, particularly for a broad implementation whose bounds are not met. It is a diagnostic hint, not a guarantee about the exact wording of every error. It does not remove the implementation, change trait resolution or bounds, disable type checking, or change runtime behavior. The attribute is intended for trait implementations; other positions do not gain this intended effect.".into(),
                            span: src.span(part), type_info: None,
                        });
                    }
                }
            }
            "line_comment" if src.text(part).starts_with("//!") => break,
            "block_comment" if src.text(part).starts_with("/*!") => break,
            "line_comment" | "block_comment" => {},
            _ => break,
        }
        previous = part.prev_sibling();
    }
    result
}

/// The IR has no standalone impl symbol. Attach direct implementations to their
/// module, their same-file exact-name type, and any symbol of a member.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "impl_item" { return item(src, node); }
    if let Some(implementation) = node.parent().and_then(|body| body.parent()).filter(|parent| parent.kind() == "impl_item") { return item(src, implementation); }
    if matches!(node.kind(), "source_file" | "mod_item") {
        let container = if node.kind() == "source_file" { Some(node) } else { node.child_by_field_name("body") };
        return container.map(|container| named_children(container).into_iter().filter(|child| child.kind() == "impl_item").flat_map(|child| item(src, child)).collect()).unwrap_or_default();
    }
    if !matches!(node.kind(), "struct_item" | "enum_item" | "union_item") { return Vec::new(); }
    let Some(name) = node.child_by_field_name("name").map(|name| src.norm(name)) else { return Vec::new(); };
    let Some(container) = node.parent() else { return Vec::new(); };
    named_children(container).into_iter().filter(|candidate| candidate.kind() == "impl_item" && candidate.child_by_field_name("type").is_some_and(|ty| src.norm(ty) == name)).flat_map(|candidate| item(src, candidate)).collect()
}
