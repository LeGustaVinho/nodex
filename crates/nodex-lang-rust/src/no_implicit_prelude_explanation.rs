//! Explain an inner `#![no_implicit_prelude]` on a crate or module.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    let mut items = Vec::new();
    for child in children(container) {
        if child.kind() == "{" || child.is_extra() { continue; }
        if child.kind() != "inner_attribute_item" { break; }
        if child.has_error() { continue; }
        let Some(meta) = named_children(child).into_iter().find(|part| part.kind() == "attribute") else { continue; };
        let Some(name) = named_children(meta).into_iter().find(|part| part.kind() == "identifier") else { continue; };
        if src.text(name) != "no_implicit_prelude" || meta.child_by_field_name("arguments").is_some() || meta.child_by_field_name("value").is_some() { continue; }
        let scope = if node.kind() == "source_file" {
            "At a crate root it affects the crate's modules. In a source file for a module it applies to that module and its descendants."
        } else {
            "Inside a module it applies to that module and its descendants."
        };
        items.push(ContractItem {
            group: "Disable implicit prelude lookups".into(),
            text: format!("#![no_implicit_prelude] is an inner attribute, so it applies to its enclosing crate or module. {} It prevents the standard-library prelude, extern prelude, macro_use prelude and tool prelude from being brought into scope implicitly there. Names that would otherwise come from those preludes may need explicit imports or paths. This does not by itself remove the standard library or prevent explicit imports. Some compiler versions still make particular built-in macros available; do not infer that every unqualified macro is absent.", scope),
            span: src.span(child), type_info: None,
        });
    }
    items
}
