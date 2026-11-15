//! Explain `#[macro_export]` on a macro_rules definition.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "macro_definition" { return Vec::new(); }
    let Some(name) = node.child_by_field_name("name") else { return Vec::new(); };
    let macro_name = src.norm(name);
    let mut previous = node.prev_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" if !part.has_error() => {
                let Some(meta) = named_children(part).into_iter().find(|child| child.kind() == "attribute") else { break; };
                let Some(path) = named_children(meta).into_iter().next() else { break; };
                if src.norm(path) == "macro_export" {
                    let arguments = meta.child_by_field_name("arguments").map(|args| src.norm(args));
                    if arguments.is_none() || arguments.as_deref() == Some("(local_inner_macros)") {
                        let local_inner = if arguments.is_some() { " The `local_inner_macros` option also prefixes single-segment macro calls in this definition with `$crate::`; it is mainly a migration aid." } else { "" };
                        result.push(ContractItem {
                            group: "Export a declarative macro".into(),
                            text: format!("`#[macro_export]` makes this `macro_rules!` macro public and available for path-based lookup from the crate root, even when the definition is inside a module. Within this crate it can be referred to as `crate::{macro_name}!`, and another crate can use a path such as `crate_name::{macro_name}!` when the crate is available. Textual scope and ordinary path resolution still matter; this attribute does not automatically import the name into every scope. It marks the definition for export rather than calling the macro or executing its expansion. Conditional compilation and errors can still affect whether the macro is present.{local_inner}"),
                            span: src.span(part), type_info: None,
                        });
                    }
                }
            }
            "line_comment" if !src.text(part).starts_with("//!") => {},
            "block_comment" if !src.text(part).starts_with("/*!") => {},
            _ => break,
        }
        previous = part.prev_sibling();
    }
    result.reverse();
    result
}
