//! Explain the `#[inline(always)]` code-generation hint.
//! Reference: https://doc.rust-lang.org/reference/attributes/codegen.html#the-inline-attribute
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let has_body = node.child_by_field_name("body").is_some();
    let mut previous = node.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = attribute.prev_named_sibling();
        if attribute.kind() != "attribute_item" || attribute.has_error() { continue; }
        let Some(meta) = named_children(attribute).into_iter().find(|part| part.kind() == "attribute") else { continue; };
        let Some(name) = named_children(meta).into_iter().find(|part| part.kind() == "identifier") else { continue; };
        if src.text(name) != "inline" { continue; }
        let Some(args) = meta.child_by_field_name("arguments") else { continue; };
        let parts: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra() && !matches!(part.kind(), "(" | ")")).collect();
        if parts.len() != 1 || src.text(parts[0]) != "always" { continue; }
        let position = if has_body {
            "For a function with a body, this requests inline expansion at call sites."
        } else {
            "This declaration has no function body. Rust specifies the hint for functions with bodies; rustc ignores and lints unsupported positions."
        };
        result.push(ContractItem {
            group: "Always-inline hint".into(),
            text: format!("#[inline(always)] suggests placing a copy of the function's code at a call site instead of generating an ordinary call. {position} Despite the word always, it is a hint and the compiler may ignore it; this text does not prove that inlining occurred or improved performance. Inlining can increase code size or slow a program. For a trait method it applies only to a default body, and for an async function it applies to the generated poll function. Rust ignores the hint on functions externally exported with no_mangle or export_name; inline(always) cannot be combined with target_feature."),
            span: src.span(attribute),
            type_info: None,
        });
    }
    result.reverse();
    result
}
