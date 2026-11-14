//! Explain simple combinations of supertraits without guessing name resolution.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "trait_item" { return None; }
    let name = node.child_by_field_name("name")?;
    let bounds = node.child_by_field_name("bounds")?;
    let written = src.norm(bounds);
    let parts: Vec<_> = written.trim().strip_prefix(':')?.split('+').map(str::trim).collect();
    // Restrict this explanation to plain names, where splitting at `+` is unambiguous.
    if parts.len() < 2 || parts.iter().any(|part| part.is_empty() || !part.chars().all(|c| c == '_' || c.is_alphanumeric())) {
        return None;
    }
    let send_sync = if parts.contains(&"Send") && parts.contains(&"Sync") {
        " If these names refer to Rust's standard traits, Send means ownership of a value can be transferred safely between threads, while Sync means shared references to it can be sent safely between threads. These names can be shadowed, so their meaning depends on name resolution."
    } else { "" };
    Some(ContractItem {
        group: "Multiple supertraits".into(),
        text: format!("In trait {}{}, the colon starts the requirements and + means all of the listed requirements apply together. A type implementing {} must also implement each of: {}. These are requirements on the implementing type, not fields or values added to the trait.{}", src.norm(name), written, src.norm(name), parts.join(", "), send_sync),
        span: src.span(bounds),
        type_info: None,
    })
}
