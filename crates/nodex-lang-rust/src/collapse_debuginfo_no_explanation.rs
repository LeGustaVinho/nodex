//! Explain `#[collapse_debuginfo(no)]` on declarative macro definitions.
//! Reference: https://doc.rust-lang.org/reference/attributes/debugger.html#the-collapse_debuginfo-attribute
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "macro_definition" { return Vec::new(); }
    let Some(macro_name) = node.child_by_field_name("name") else { return Vec::new(); };
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
        if parts.len() != 1 || src.text(parts[0]) != "no" { continue; }
        items.push(ContractItem {
            group: "Macro debug-location mapping".into(),
            text: format!("#[collapse_debuginfo(no)] applies to this macro_rules! definition {}. It requests that locations from the macro definition not be collapsed into a single location at the invocation site when generating debug information. This can affect the source locations a debugger shows for expanded code; it does not change macro matching, expansion, or the program's runtime behavior. The rustc -C collapse-macro-debuginfo option can override the attribute. Source syntax alone does not prove debug information is emitted or how a particular debugger will step through the expansion.", src.norm(macro_name)),
            span: src.span(attribute),
            type_info: None,
        });
    }
    items.reverse();
    items
}
