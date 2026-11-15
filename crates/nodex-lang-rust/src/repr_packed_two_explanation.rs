//! Explain the two-byte packing modifier on Rust struct and union layouts.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn packed_two<'t>(src: Src<'_>, attribute: Node<'t>) -> Option<(Node<'t>, Node<'t>)> {
    if attribute.kind() != "attribute_item" || attribute.has_error() { return None; }
    let attr = named_children(attribute).into_iter().find(|part| part.kind() == "attribute")?;
    let name = named_children(attr).into_iter().find(|part| part.kind() == "identifier")?;
    if src.text(name) != "repr" { return None; }
    let args = attr.child_by_field_name("arguments")?;
    let tokens: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
    if tokens.len() < 4 || tokens.first()?.kind() != "(" || tokens.last()?.kind() != ")" { return None; }
    for i in 1..tokens.len()-2 {
        let (word, group) = (tokens[i], tokens[i+1]);
        if word.kind() != "identifier" || src.text(word) != "packed" || group.kind() != "token_tree" { continue; }
        if !matches!(tokens[i-1].kind(), "(" | ",") || !matches!(tokens[i+2].kind(), ")" | ",") { continue; }
        let inner: Vec<_> = children(group).into_iter().filter(|part| !part.is_extra()).collect();
        if inner.len() == 3 && inner[0].kind() == "(" && inner[1].kind() == "integer_literal"
            && src.text(inner[1]) == "2" && inner[2].kind() == ")" { return Some((word, group)); }
    }
    None
}

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "struct_item" | "union_item") { return Vec::new(); }
    let mut previous = declaration.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = attribute.prev_named_sibling();
        let Some((word, group)) = packed_two(src, attribute) else { continue };
        result.push(ContractItem {
            group: "Two-byte packed representation".into(),
            text: "#[repr(packed(2))] changes the layout of this struct or union: for placing each field, Rust uses the smaller of that field type's alignment and 2 bytes. It can reduce the type's alignment and inter-field padding, but does not increase alignment, does not necessarily remove all padding, and by itself does not guarantee field order or byte order. Consequently a field may be at an address insufficiently aligned for its own type. Creating an unaligned & or &mut reference is invalid even inside unsafe; implicit references, such as formatting a field, also matter. When appropriate, copy a Copy field by value into a local, or form a raw pointer without first creating a reference and use an unaligned read/write only after satisfying its safety requirements. Combining repr(C) changes the layout guarantees; the attribute alone does not prove a particular field offset.".into(),
            span: src.index.span(src.file, word.start_byte(), group.end_byte()), type_info: None,
        });
    }
    result
}
