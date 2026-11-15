//! Explain `#[collapse_debuginfo(yes)]` on a declarative macro definition.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "macro_definition" { return Vec::new(); }
    let mut previous = node.prev_named_sibling();
    let mut items = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = attribute.prev_named_sibling();
        if attribute.kind() != "attribute_item" || attribute.has_error() { continue; }
        let Some(meta) = named_children(attribute).into_iter().find(|part| part.kind() == "attribute") else { continue; };
        let Some(name) = named_children(meta).into_iter().find(|part| part.kind() == "identifier") else { continue; };
        if src.text(name) != "collapse_debuginfo" { continue; }
        let Some(args) = meta.child_by_field_name("arguments") else { continue; };
        let parts: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra() && !matches!(part.kind(), "(" | ")")).collect();
        if parts.len() != 1 || src.text(parts[0]) != "yes" { continue; }
        items.push(ContractItem {
            group: "Collapse macro debug locations".into(),
            text: "#[collapse_debuginfo(yes)] on this macro_rules! definition asks the compiler to collapse source locations from the macro definition into the macro call-site location in generated debug information. A debugger may therefore show the invocation rather than stepping through individual locations in the expanded macro. The no option requests that locations not be collapsed; external collapses them only for macros from a different crate. The compiler's collapse-macro-debuginfo codegen option can override the attribute. Actual debugger behavior also depends on available debug information and the debugger; this attribute does not change the macro's Rust program behavior.".into(),
            span: src.span(attribute), type_info: None,
        });
    }
    items.reverse();
    items
}
