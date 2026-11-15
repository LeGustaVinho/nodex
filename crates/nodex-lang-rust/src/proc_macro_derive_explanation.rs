//! Explain the outer attribute declaring a derive procedural macro.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_item" { return Vec::new(); }
    let mut previous = node.prev_named_sibling();
    let mut items = Vec::new();
    while let Some(part) = previous {
        if part.kind() == "attribute_item" {
            if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute") {
                let path = named_children(attribute).into_iter().next();
                if path.is_some_and(|path| src.norm(path) == "proc_macro_derive")
                    && let Some(arguments) = attribute.child_by_field_name("arguments")
                    && let Some(name) = named_children(arguments).into_iter().find(|child| !child.is_extra())
                    && matches!(name.kind(), "identifier" | "type_identifier")
                {
                    let macro_name = src.norm(name);
                    let function = node.child_by_field_name("name").map(|name| src.norm(name)).unwrap_or_else(|| "this function".into());
                    items.push(ContractItem {
                        group: "Derive procedural macro".into(),
                        text: format!("`#[proc_macro_derive({macro_name})]` declares {macro_name} as the name used in `#[derive({macro_name})]`; {function} is the Rust function that implements this macro, so the two names need not match. When a valid derive is applied to a struct, enum, or union, the compiler invokes the macro during compilation with the item's tokens. The macro returns tokens for additional items appended after that item in the same module. This is code generation, not a runtime function call on each value. Rust checks requirements including a procedural-macro crate, a public function with the required TokenStream signature, and the expansion's validity; the attribute alone does not prove those requirements are satisfied."),
                        span: src.span(part), type_info: None,
                    });
                }
            }
        } else if !part.is_extra() { break; }
        previous = part.prev_named_sibling();
    }
    items.reverse();
    items
}
