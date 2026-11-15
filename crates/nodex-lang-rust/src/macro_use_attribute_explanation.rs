//! Explain the two `#[macro_use]` forms without conflating their scopes.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn attributes<'t>(src: Src<'_>, item: Node<'t>) -> Vec<Node<'t>> {
    let mut previous = item.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        if part.kind() == "attribute_item" {
            if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute")
                && named_children(attribute).into_iter().next().is_some_and(|path| src.norm(path) == "macro_use")
            { result.push(part); }
        } else if !part.is_extra() { break; }
        previous = part.prev_named_sibling();
    }
    result.reverse();
    result
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let mut result = Vec::new();
    if node.kind() == "mod_item" {
        let name = node.child_by_field_name("name").map(|part| src.norm(part)).unwrap_or_else(|| "this module".into());
        for attr in attributes(src, node) {
            result.push(ContractItem {
                group: "Import macros from a module".into(),
                text: format!("`#[macro_use]` on module {name} extends the textual scope of eligible macros defined inside that module into its enclosing module, after the module declaration. This affects how unqualified macro names can be found; it does not call or expand those macros at the attribute. Visibility, ordering and name resolution still matter. This module form differs from `#[macro_use] extern crate ...`, which imports external macros into the macro-use prelude."),
                span: src.span(attr), type_info: None,
            });
        }
    }
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    if let Some(container) = container {
        for item in named_children(container).into_iter().filter(|child| child.kind() == "extern_crate_declaration") {
            let name = item.child_by_field_name("name").map(|part| src.norm(part)).unwrap_or_else(|| "the external crate".into());
            for attr in attributes(src, item) {
                result.push(ContractItem {
                    group: "Import macros from an external crate".into(),
                    text: format!("{} on `extern crate {name}` is the legacy external-crate form. When accepted at the crate root, it imports eligible exported macros from that crate into the macro-use prelude so they can be found by unqualified name; a listed set of names restricts the import. This is different from extending a module's textual macro scope. The attribute does not invoke a macro, and the compiler checks the crate, placement, names and visibility.", src.norm(attr)),
                    span: src.span(attr), type_info: None,
                });
            }
        }
    }
    result
}
