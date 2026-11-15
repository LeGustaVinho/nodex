//! Explain the external-macro debug location policy.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "macro_definition" { return Vec::new(); }
    let mut previous = node.prev_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" if !part.has_error() => {
                let Some(meta) = named_children(part).into_iter().find(|child| child.kind() == "attribute") else { break; };
                let Some(path) = named_children(meta).into_iter().next() else { break; };
                if src.norm(path) == "collapse_debuginfo" && meta.child_by_field_name("arguments").is_some_and(|args| src.norm(args) == "(external)") {
                    result.push(ContractItem {
                        group: "Macro debug locations for external callers".into(),
                        text: "`#[collapse_debuginfo(external)]` on this `macro_rules!` definition requests that, when the macro is called from a different crate, locations from its expanded code be collapsed to its call site in generated debug information. A debugger may therefore show the call rather than individual expansion locations. Calls in the macro's own crate do not get collapsed under this setting. This changes debugging locations, not what the macro executes. rustc's `-C collapse-macro-debuginfo` option can override the attribute, and debug information depends on the build configuration.".into(),
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
