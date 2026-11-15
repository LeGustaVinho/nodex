//! Explain a Windows target-OS conditional compilation attribute.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if matches!(declaration.kind(), "source_file" | "attribute_item" | "inner_attribute_item") { return Vec::new(); }
    let mut previous = declaration.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(item) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = item.prev_named_sibling();
        if item.kind() != "attribute_item" || item.has_error() { continue; }
        let Some(attr) = named_children(item).into_iter().find(|part| part.kind() == "attribute") else { continue };
        let Some(name) = named_children(attr).into_iter().find(|part| part.kind() == "identifier") else { continue };
        if src.text(name) != "cfg" { continue; }
        let Some(args) = attr.child_by_field_name("arguments") else { continue };
        let tokens: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
        if tokens.len() != 5 || tokens[0].kind() != "(" || tokens[1].kind() != "identifier"
            || src.text(tokens[1]) != "target_os" || tokens[2].kind() != "="
            || tokens[3].kind() != "string_literal" || src.text(tokens[3]) != "\"windows\""
            || tokens[4].kind() != ")" { continue; }
        result.push(ContractItem {
            group: "Windows target build condition".into(),
            text: "This outer #[cfg(target_os = \"windows\")] attribute applies to the following item. Rust includes the item when the compilation target's configuration contains target_os = \"windows\"; otherwise this condition removes the item from that build. It is evaluated as build configuration, not as a runtime operating-system check. The compilation target can differ from the machine running Nodex or the compiler, for example during cross-compilation. This source text alone does not establish whether the currently selected build satisfies the condition.".into(),
            span: src.span(item), type_info: None,
        });
    }
    result
}
