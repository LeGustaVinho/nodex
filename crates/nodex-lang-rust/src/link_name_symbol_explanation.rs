//! Explain an external function's explicitly named linker symbol.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_signature_item" { return Vec::new(); }
    if !node.parent().and_then(|body| body.parent()).is_some_and(|parent| parent.kind() == "foreign_mod_item") { return Vec::new(); }
    let Some(name) = node.child_by_field_name("name") else { return Vec::new(); };
    let mut previous = node.prev_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" if !part.has_error() => {
                if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute")
                    && let Some(path) = named_children(attribute).into_iter().next()
                    && src.text(path) == "link_name"
                    && let Some(value) = attribute.child_by_field_name("value")
                    && matches!(value.kind(), "string_literal" | "raw_string_literal")
                    && (src.text(value).starts_with('"') || src.text(value).starts_with('r'))
                {
                    result.push(ContractItem {
                        group: "Foreign linker symbol name".into(),
                        text: format!("`#[link_name = ...]` tells Rust to import the foreign symbol named by {} for this declaration. Rust code still refers to the function by its Rust name, {}; the specified string is the external symbol name, not a new Rust function or a library name. The enclosing extern block determines the calling convention, and linking must still supply a matching symbol. This attribute alone does not verify the library, ABI, signature or safety of calling it. The string is shown as written; Rust checks that the resulting symbol name is nonempty and contains no NUL bytes.", src.text(value), src.norm(name)),
                        span: src.span(part), type_info: None,
                    });
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
