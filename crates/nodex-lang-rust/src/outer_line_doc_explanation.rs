//! Explain outer line documentation comments attached to an item.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

/// A line beginning with exactly three slashes is an outer doc comment.
/// Four or more slashes form an ordinary line comment instead.
fn outer_doc_line(src: Src<'_>, node: Node<'_>) -> bool {
    if node.kind() != "line_comment" { return false; }
    let text = src.text(node);
    text.starts_with("///") && !text.starts_with("////")
}

/// Attach the explanation to the declaration's own visible symbol contract.
/// Comments and outer attributes are trivia between documentation and its item;
/// an inner doc comment belongs to the surrounding item and stops the search.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "enum_variant" | "union_item" | "trait_item" | "type_item" | "associated_type" | "const_item" | "static_item" | "field_declaration" | "mod_item" | "macro_definition") { return None; }
    let mut previous = node.prev_sibling();
    let mut nearest = None;
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" => {},
            "line_comment" if src.text(part).starts_with("//!") => break,
            "line_comment" => { if nearest.is_none() && outer_doc_line(src, part) { nearest = Some(part); } },
            "block_comment" if src.text(part).starts_with("/*!") => break,
            "block_comment" => {},
            _ => break,
        }
        previous = part.prev_sibling();
    }
    let comment = nearest?;
    let span = src.index.span(src.file, comment.start_byte(), comment.start_byte() + 3);
    Some(ContractItem {
        group: "Outer documentation comment".into(),
        text: "A line beginning with /// is an outer documentation comment for the following item. Rust treats its text as documentation metadata (like an outer #[doc = ...] attribute), rather than executing the text as code. Rustdoc normally renders the text as Markdown and can test eligible Rust code examples as documentation tests. Multiple doc-comment lines can contribute to the same item. //! instead documents the enclosing item; //// is an ordinary line comment.".into(),
        span, type_info: None,
    })
}
