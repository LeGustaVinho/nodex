//! Explain the default safety of functions declared in an extern block.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// The parser may blank the `safe` keyword for grammar compatibility, so look
/// in the original source before `fn`, excluding separate comments/attributes.
fn has_written_safety(src: Src<'_>, node: Node<'_>, keyword: Node<'_>) -> bool {
    let mut previous = node.prev_sibling();
    let mut ignored = Vec::new();
    while let Some(part) = previous {
        if matches!(part.kind(), "attribute_item" | "line_comment" | "block_comment") {
            ignored.push((part.start_byte(), part.end_byte()));
            previous = part.prev_sibling();
        } else { break; }
    }
    let start = previous.map(|part| part.end_byte()).unwrap_or_else(|| node.parent().map_or(0, |parent| parent.start_byte()));
    let end = keyword.start_byte();
    let bytes = src.text.as_bytes();
    let mut i = start.min(end);
    while i < end {
        if let Some((_, finish)) = ignored.iter().find(|(begin, finish)| *begin <= i && i < *finish) { i = *finish; continue; }
        if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' {
            let begin = i;
            i += 1;
            while i < end && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1; }
            if matches!(&src.text[begin..i], "safe" | "unsafe") { return true; }
        } else { i += 1; }
    }
    false
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_signature_item" || node.has_error() { return Vec::new(); }
    if !node.parent().and_then(|body| body.parent()).is_some_and(|owner| owner.kind() == "foreign_mod_item") { return Vec::new(); }
    let Some(keyword) = children(node).into_iter().find(|child| child.kind() == "fn") else { return Vec::new(); };
    if has_written_safety(src, node, keyword) { return Vec::new(); }
    let Some(name) = node.child_by_field_name("name") else { return Vec::new(); };
    vec![ContractItem {
        group: "Foreign function unsafe by default".into(),
        text: format!("{} is declared in an extern block without an explicit `safe` or `unsafe` qualifier. Such a foreign function is unsafe to call by default: a call needs an unsafe context, and the caller must uphold its documented safety requirements. `unsafe extern` on the block emphasizes the declaration author's responsibility for correct signatures and ABI; it does not make every call safe. In an unsafe extern block, an explicitly `safe fn` can be called without an unsafe context when its declaration's safety promise is valid. The declaration alone cannot verify the external implementation or its contract.", src.norm(name)),
        span: src.span(keyword), type_info: None,
    }]
}
