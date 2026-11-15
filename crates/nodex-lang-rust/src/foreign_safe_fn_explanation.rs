//! Explain explicitly safe functions in a foreign block, preserving `safe`
//! even when the Rust grammar's compatibility rewrite blanks that keyword.
use nodex_ir::{reading::ContractItem, Span};
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Return the original `safe` span and foreign block only when the written
/// keyword immediately precedes `fn` (whitespace may intervene). This avoids
/// interpreting comments or an `unsafe` suffix as the safe qualifier.
fn safe_keyword<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Span, Node<'t>)> {
    if node.kind() != "function_signature_item" || node.has_error() { return None; }
    let body = node.parent()?;
    let foreign = body.parent().filter(|p| p.kind() == "foreign_mod_item")?;
    let fn_token = children(node).into_iter().find(|part| part.kind() == "fn" && !part.is_missing())?;
    let lower = node.prev_sibling().map_or(body.start_byte() + 1, |previous| previous.end_byte());
    let prefix = src.text.get(lower..fn_token.start_byte())?;
    let trimmed = prefix.trim_end();
    let before = trimmed.strip_suffix("safe")?;
    if before.as_bytes().last().is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_') { return None; }
    let start = lower + before.len();
    Some((src.index.span(src.file, start, start + 4), foreign))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    let (safe, foreign) = safe_keyword(src, node)?;
    let name = node.child_by_field_name("name")?;
    let header = src.header(foreign, foreign.child_by_field_name("body"));
    Some(ContractItem {
        group: "Explicitly safe foreign function".into(),
        text: format!("{} is a declaration in the foreign block `{}`; its body is supplied outside this Rust declaration and the block specifies the foreign calling convention. The `safe` qualifier declares that safe Rust may call this function without an unsafe block, unlike foreign functions left implicitly unsafe. The author of the extern block is responsible for the correctness of the ABI, signature, and safety promise: the keyword does not verify the foreign implementation or prove it is actually safe.", src.norm(name), header),
        span: safe, type_info: None,
    })
}

/// Recover the full written signature including a blanked `safe` qualifier.
pub(crate) fn written_signature(src: Src<'_>, node: Node<'_>) -> Option<String> {
    let (safe, _) = safe_keyword(src, node)?;
    let start = node.start_byte().min(safe.start_byte as usize);
    Some(src.text.get(start..node.end_byte())?.trim().trim_end_matches(';').trim_end().to_owned())
}
