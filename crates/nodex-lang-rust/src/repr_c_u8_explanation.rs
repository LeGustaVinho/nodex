//! Explain the combination `#[repr(C, u8)]` on an enum.
//! Reference: https://doc.rust-lang.org/reference/type-layout.html#combining-primitive-representations-of-enums-with-fields-and-reprc
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "enum_item" { return Vec::new(); }
    let Some(body) = node.child_by_field_name("body") else { return Vec::new(); };
    let variants: Vec<_> = named_children(body).into_iter().filter(|part| part.kind() == "enum_variant").collect();
    let with_fields = variants.iter().any(|variant| variant.child_by_field_name("body").is_some_and(|fields| {
        named_children(fields).into_iter().any(|field| !field.is_extra() && field.kind() != "attribute_item")
    }));
    let specific = if variants.is_empty() {
        "This enum has no variants; a zero-variant enum cannot use a primitive integer representation, so the compiler rejects it."
    } else if !with_fields {
        "This enum has no fields. On a fieldless enum, rustc currently diagnoses the combination C and u8 as conflicting representation hints by default; this syntax alone is not evidence of a valid layout."
    } else {
        "For an enum with fields, Rust specifies a C-representation struct containing a tag and a C-representation union of the variant payloads. The u8 part chooses an unsigned one-byte representation for the tag (discriminant), whose size and alignment are one byte. This is not generally the same layout as repr(u8) alone."
    };
    let mut previous = node.prev_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        previous = part.prev_sibling();
        match part.kind() {
            "attribute_item" => {},
            "line_comment" if !src.text(part).starts_with("//!") => continue,
            "block_comment" if !src.text(part).starts_with("/*!") => continue,
            _ => break,
        }
        if part.has_error() { continue; }
        let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute") else { continue; };
        let Some(args) = attribute.child_by_field_name("arguments") else { continue; };
        let Some(name) = named_children(attribute).into_iter().find(|child| *child != args) else { continue; };
        if src.text(name) != "repr" { continue; }
        let tokens: Vec<_> = children(args).into_iter().filter(|child| !child.is_extra() && !matches!(child.kind(), "(" | ")" | ",")).collect();
        if tokens.len() != 2 || !tokens.iter().any(|token| src.text(*token) == "C") || !tokens.iter().any(|token| src.text(*token) == "u8") { continue; }
        result.push(ContractItem {
            group: "C layout with u8 tag".into(),
            text: format!("#[repr(C, u8)] requests a specific representation for this enum. {specific} Exact size, padding and alignment of the complete value depend on the payloads and target; u8 describes the tag, not necessarily the entire enum. This attribute alone does not prove that every field is FFI-safe or that every possible byte pattern is a valid Rust enum value."),
            span: src.span(part),
            type_info: None,
        });
    }
    result.reverse();
    result
}
