//! Explain Rust's built-in `char` when it occurs in a written type.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

const TEXT: &str = "Rust's char holds one Unicode scalar value in 4 bytes. A scalar can be an ordinary letter or an emoji, but excludes UTF-16 surrogate code points. A visible character can consist of multiple scalars (for example a letter followed by a combining accent). C# char instead holds one UTF-16 code unit, so some scalars require a surrogate pair there.";

fn chars_in_type<'a>(node: Node<'a>, out: &mut Vec<Node<'a>>) {
    if node.kind() == "primitive_type" { out.push(node); return; }
    for child in named_children(node) { chars_in_type(child, out); }
}

fn type_chars<'a>(src: Src<'_>, ty: Node<'a>) -> Vec<Node<'a>> {
    let mut nodes = Vec::new();
    chars_in_type(ty, &mut nodes);
    nodes.into_iter().filter(|node| src.text(*node) == "char").collect()
}

/// Explain type annotations on this symbol's signature, without scanning its body.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let mut types = Vec::new();
    match node.kind() {
        "function_item" | "function_signature_item" => {
            if let Some(params) = node.child_by_field_name("parameters") {
                types.extend(named_children(params).into_iter().filter_map(|p| p.child_by_field_name("type")));
            }
            types.extend(node.child_by_field_name("return_type"));
        }
        "field_declaration" | "const_item" | "static_item" | "type_item" => types.extend(node.child_by_field_name("type")),
        _ => return Vec::new(),
    }
    types.into_iter().flat_map(|ty| type_chars(src, ty)).map(|ty| ContractItem {
        group: "Character type".into(), text: TEXT.into(), span: src.span(ty), type_info: None,
    }).collect()
}

/// Attach the explanation to an explicitly typed local declaration.
pub(crate) fn local<'a>(src: Src<'_>, node: Node<'a>) -> Vec<(Node<'a>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new() };
    type_chars(src, ty).into_iter().map(|ty| (ty, ReadingContent::Detail {
        label: "Unicode character type".into(), text: TEXT.into(), borrow: false, definition: None,
    })).collect()
}
