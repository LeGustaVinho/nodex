//! Explain the `#[must_use]` diagnostic attribute on supported Rust items.
//! Reference: https://doc.rust-lang.org/reference/attributes/diagnostics.html#the-must_use-attribute
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let trait_implementation_method = matches!(node.kind(), "function_item" | "function_signature_item")
        && node.parent().and_then(|body| body.parent()).is_some_and(|owner| owner.kind() == "impl_item" && owner.child_by_field_name("trait").is_some());
    let affected = match node.kind() {
        "function_item" | "function_signature_item" if trait_implementation_method => "On a method directly inside a trait implementation, this attribute has no effect and rustc lints this placement. A must_use attribute on the trait's method declaration can apply to implementations.",
        "function_item" | "function_signature_item" => "When a call to this function or method is used as a statement and its result is ignored, Rust can report the unused_must_use lint.",
        "struct_item" | "enum_item" | "union_item" => "When an expression statement produces a value of this type and discards it, Rust can report the unused_must_use lint.",
        "trait_item" => "When a function call returning an opaque impl Trait or a dyn trait object with this trait in its bounds is discarded as a statement, Rust can report the unused_must_use lint.",
        _ => return Vec::new(),
    };
    let mut previous = node.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = attribute.prev_named_sibling();
        if attribute.kind() != "attribute_item" || attribute.has_error() { continue; }
        let Some(meta) = named_children(attribute).into_iter().find(|part| part.kind() == "attribute") else { continue; };
        let Some(name) = named_children(meta).into_iter().find(|part| part.kind() == "identifier") else { continue; };
        if src.text(name) != "must_use" || meta.child_by_field_name("arguments").is_some() { continue; }
        let message = match meta.child_by_field_name("value") {
            Some(value) if matches!(value.kind(), "string_literal" | "raw_string_literal") => format!(" The attribute supplies {} as a message that can be included in the diagnostic.", src.norm(value)),
            Some(_) => continue,
            None => String::new(),
        };
        result.push(ContractItem {
            group: "Must-use diagnostic".into(),
            text: format!("#[must_use] marks a result or value as important to use. {affected}{message} This is a diagnostic lint, not proof that the program handled the value: lint settings can allow, warn, or deny it, and an intentional discard such as let _ = value can be explicit. The attribute does not itself execute or inspect the result."),
            span: src.span(attribute),
            type_info: None,
        });
    }
    result.reverse();
    result
}
