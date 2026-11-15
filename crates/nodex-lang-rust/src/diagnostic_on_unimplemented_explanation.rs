//! Explain the trait diagnostic hint without evaluating compiler behavior.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::{kit::truncate, syntax::{named_children, Src}};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "trait_item" { return Vec::new(); }
    let Some(trait_name) = node.child_by_field_name("name") else { return Vec::new(); };
    let mut previous = node.prev_named_sibling();
    let mut items = Vec::new();
    while let Some(part) = previous {
        if part.kind() == "attribute_item" {
            if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute")
                && let Some(path) = named_children(attribute).into_iter().next()
                && src.text(path).chars().filter(|ch| !ch.is_whitespace()).collect::<String>() == "diagnostic::on_unimplemented"
            {
                items.push(ContractItem {
                    group: "Trait diagnostic hint".into(),
                    text: format!("{} is a diagnostic hint attached to the trait {}. When a type is required to implement this trait but does not, a compiler may use the attribute to customize its error message. Options such as `message`, `label`, and `note` provide text for the main message, inline label, and additional note; supported placeholders such as `{{Self}}` can refer to the type involved. This is a request about diagnostics: it does not implement the trait, relax the bound, or change runtime behavior. The compiler and the written options determine what is displayed, so this attribute alone does not guarantee a particular message.", truncate(&src.norm(part), 180), src.norm(trait_name)),
                    span: src.span(part), type_info: None,
                });
            }
        } else if !part.is_extra() { break; }
        previous = part.prev_named_sibling();
    }
    items.reverse();
    items
}
