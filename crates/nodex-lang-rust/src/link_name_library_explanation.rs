//! Explain a native-library `#[link(name = ...)]` on a foreign block.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

/// Attach the enclosing block's link attribute to each represented foreign function.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_signature_item" { return Vec::new(); }
    let Some(block) = node.parent().and_then(|body| body.parent()).filter(|item| item.kind() == "foreign_mod_item") else { return Vec::new(); };
    let mut previous = block.prev_named_sibling();
    let mut items = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = attribute.prev_named_sibling();
        if attribute.kind() != "attribute_item" || attribute.has_error() { continue; }
        let Some(meta) = named_children(attribute).into_iter().find(|part| part.kind() == "attribute") else { continue; };
        let Some(name) = named_children(meta).into_iter().find(|part| part.kind() == "identifier") else { continue; };
        if src.text(name) != "link" { continue; }
        let Some(args) = meta.child_by_field_name("arguments") else { continue; };
        let parts: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra() && !matches!(part.kind(), "(" | ")")).collect();
        for (index, group) in parts.windows(3).enumerate() {
            let [key, equal, value] = group else { continue; };
            if src.text(*key) != "name" || equal.kind() != "=" || !matches!(value.kind(), "string_literal" | "raw_string_literal") { continue; }
            if index > 0 && parts[index-1].kind() != "," { continue; }
            if index+3 < parts.len() && parts[index+3].kind() != "," { continue; }
            items.push(ContractItem {
                group: "Native library link".into(),
                text: format!("{} requests linking the native library named by {} for declarations in this extern block. For example, the foreign functions declared here may be supplied by that library. The actual library file, search path and link behavior depend on the target, toolchain and any other link options; the attribute does not guarantee that the library is installed or linking succeeds. Linking a library is separate from choosing a calling convention and does not by itself prove that calling a foreign function is safe.", src.norm(attribute), src.text(*value)),
                span: src.span(attribute), type_info: None,
            });
            break;
        }
    }
    items.reverse();
    items
}
