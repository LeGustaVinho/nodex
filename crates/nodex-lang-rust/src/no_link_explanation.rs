//! Explain `#[no_link]` on a direct `extern crate` declaration.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn attribute<'t>(src: Src<'_>, declaration: Node<'t>) -> Option<Node<'t>> {
    let mut previous = declaration.prev_sibling();
    let mut first = None;
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" => {
                if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute") {
                    let path = named_children(attribute).into_iter().next();
                    if path.is_some_and(|path| src.norm(path) == "no_link") && attribute.child_by_field_name("arguments").is_none() && attribute.child_by_field_name("value").is_none() { first = Some(part); }
                }
            }
            "line_comment" if src.text(part).starts_with("//!") => break,
            "block_comment" if src.text(part).starts_with("/*!") => break,
            "line_comment" | "block_comment" => {},
            _ => break,
        }
        previous = part.prev_sibling();
    }
    first
}

/// An extern crate has no separate symbol, so use its enclosing module's contract.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    named_children(container).into_iter().filter_map(|declaration| {
        if declaration.kind() != "extern_crate_declaration" || declaration.has_error() { return None; }
        let name = declaration.child_by_field_name("name")?;
        let attr = attribute(src, declaration)?;
        Some(ContractItem {
            group: "External crate without linking".into(),
            text: format!("`#[no_link]` on this `extern crate {}` declaration tells the compiler not to link the crate through this declaration. The crate still has to be available to the build and may be referenced, for example to use its macros. Using runtime symbols that require the omitted linkage may lead to link errors. Other declarations or dependencies can affect the final artifact, so this attribute alone does not prove that no code from the crate is linked anywhere. It is a legacy attribute, and compiler versions may lint or reject its use. It is intended for `extern crate`, not an `extern` function or block.", src.norm(name)),
            span: src.span(attr), type_info: None,
        })
    }).collect()
}
