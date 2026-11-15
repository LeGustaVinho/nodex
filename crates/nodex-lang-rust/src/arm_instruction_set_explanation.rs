//! Explain direct ARM A32 instruction-set selection on a function with a body.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn is_arm_a32(src: Src<'_>, item: Node<'_>) -> bool {
    if item.kind() != "attribute_item" || item.has_error() { return false; }
    let Some(attr) = named_children(item).into_iter().find(|part| part.kind() == "attribute") else { return false };
    let Some(name) = named_children(attr).into_iter().find(|part| part.kind() == "identifier") else { return false };
    if src.text(name) != "instruction_set" { return false; }
    let Some(args) = attr.child_by_field_name("arguments") else { return false };
    let tokens: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
    if tokens.len() < 3 || tokens.first().is_none_or(|part| part.kind() != "(") || tokens.last().is_none_or(|part| part.kind() != ")") { return false; }
    let mut path = &tokens[1..tokens.len()-1];
    if path.last().is_some_and(|part| part.kind() == ",") { path = &path[..path.len()-1]; }
    if path.is_empty() || path.iter().any(|part| !matches!(part.kind(), "identifier" | "scoped_identifier" | "::")) { return false; }
    let written: String = path.iter().flat_map(|part| src.text(*part).chars()).filter(|ch| !ch.is_whitespace()).collect();
    written == "arm::a32"
}

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    // The attribute is specified for functions with bodies; declarations without
    // a body and other item kinds cannot be treated as generated functions.
    if declaration.kind() != "function_item" || declaration.child_by_field_name("body").is_none() { return Vec::new(); }
    let mut previous = declaration.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = attribute.prev_named_sibling();
        if !is_arm_a32(src, attribute) { continue; }
        result.push(ContractItem {
            group: "ARM A32 instruction set".into(),
            text: "#[instruction_set(arm::a32)] selects the ARM A32 instruction set for code generated for this function, instead of relying solely on the target's default instruction set. It is a code-generation attribute, not a runtime CPU check or a call to the function. The compilation target and toolchain must support this value; for example the Rust Reference lists ARMv4T and ARMv5te as supporting A32 and T32, while not every target supports A32. Source spelling alone does not establish which target is selected or whether the resulting code is compatible with the processor that will run it. Inline assembly in this function must use the selected instruction set. arm::t32 selects Thumb/T32 where supported; this attribute may be used only once on a function.".into(),
            span: src.span(attribute), type_info: None,
        });
    }
    result
}
