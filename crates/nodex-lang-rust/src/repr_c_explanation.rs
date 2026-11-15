//! Explain the direct `#[repr(C)]` layout attribute according to item kind.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let type_detail = match node.kind() {
        "struct_item" => "For this struct, the C representation places fields in declaration order with target-dependent alignment and padding.",
        "union_item" => "For this union, the fields overlap at the same starting offset; its C representation uses the target's field sizes and alignments. The union does not gain a tag that records which field was written.",
        "enum_item" => "For an enum without payload fields, Rust uses a target-specific estimate of the C enum representation; C compiler flags can change the C side. For an enum with payload fields, Rust specifies a C-representation tag and payload layout. Not every possible tag or bit pattern is necessarily a valid Rust enum value.",
        _ => return Vec::new(),
    };
    let mut previous = node.prev_sibling();
    let mut output = Vec::new();
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
        let Some(arguments) = attribute.child_by_field_name("arguments") else { continue; };
        let Some(path) = named_children(attribute).into_iter().find(|child| *child != arguments) else { continue; };
        if src.text(path) != "repr" { continue; }
        let tokens: Vec<_> = children(arguments).into_iter().filter(|child| !child.is_extra()).collect();
        let [open, representation, rest @ ..] = tokens.as_slice() else { continue; };
        if open.kind() != "(" || src.text(*representation) != "C" { continue; }
        if !matches!(rest, [close] if close.kind() == ")") && !matches!(rest, [comma, close] if comma.kind() == "," && close.kind() == ")") { continue; }
        let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_else(|| "this type".into());
        output.push(ContractItem {
            group: "C representation".into(),
            text: format!("`#[repr(C)]` requests Rust's C representation for {name}. {type_detail} The precise layout depends on the compilation target. This attribute alone does not make every field FFI-safe or determine the layout of nested types; check their representations and the foreign interface separately."),
            span: src.span(part), type_info: None,
        });
    }
    output
}
