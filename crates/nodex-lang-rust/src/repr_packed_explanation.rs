//! Explain the default one-byte packed representation on structs and unions.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

/// Recognize `packed` and the equivalent `packed(1)` as direct repr entries.
/// Other packing factors, nested cfg_attr and malformed attributes differ.
fn packed_one<'t>(src: Src<'_>, attribute: Node<'t>) -> Option<Node<'t>> {
    if attribute.kind() != "attribute_item" || attribute.has_error() { return None; }
    let attr = named_children(attribute).into_iter().find(|part| part.kind() == "attribute")?;
    let name = named_children(attr).into_iter().find(|part| part.kind() == "identifier")?;
    if src.text(name) != "repr" { return None; }
    let args = attr.child_by_field_name("arguments")?;
    if args.kind() != "token_tree" || args.has_error() { return None; }
    let parts: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
    if parts.len() < 3 || parts.first()?.kind() != "(" || parts.last()?.kind() != ")" { return None; }
    for entry in parts[1..parts.len()-1].split(|part| part.kind() == ",") {
        match entry {
            [packed] if packed.kind() == "identifier" && src.text(*packed) == "packed" => return Some(*packed),
            [packed, arg] if packed.kind() == "identifier" && src.text(*packed) == "packed" && arg.kind() == "token_tree" && !arg.has_error() => {
                let inner: Vec<_> = children(*arg).into_iter().filter(|part| !part.is_extra()).collect();
                if matches!(inner.as_slice(), [open, one, close] if open.kind() == "(" && one.kind() == "integer_literal" && src.text(*one) == "1" && close.kind() == ")") { return Some(*packed); }
            }
            _ => {}
        }
    }
    None
}

/// Show the packed type's explanation on the type and its direct field symbols.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let owner = if matches!(node.kind(), "struct_item" | "union_item") {
        Some(node)
    } else {
        node.parent().filter(|body| matches!(body.kind(), "field_declaration_list" | "ordered_field_declaration_list"))
            .and_then(|body| body.parent()).filter(|parent| matches!(parent.kind(), "struct_item" | "union_item"))
    };
    let Some(owner) = owner else { return Vec::new(); };
    let mut previous = owner.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        if let Some(packed) = packed_one(src, attribute) {
            result.push(ContractItem {
                group: "Packed representation".into(),
                text: "For a valid struct or union, repr(packed) means repr(packed(1)): its alignment is one byte, and fields are placed using at most one-byte alignment. A packed struct has no padding between its fields, although padding inside a field's own type remains. Packed alone does not guarantee field order or select the C representation; repr(C, packed) requests the C representation separately. Fields may therefore be unaligned for their types. An unaligned reference is undefined behavior, and Rust rejects known attempts to create one, even temporarily. An eligible Copy field can be copied by value; alternatively, a raw pointer such as &raw const value.field and read_unaligned may be used where appropriate, with all unsafe validity and ownership requirements satisfied. The compiler still checks that representation modifiers are valid together.".into(),
                span: src.span(packed), type_info: None,
            });
        }
        previous = attribute.prev_named_sibling();
    }
    result
}
