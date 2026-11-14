use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

/// Explain raw identifiers in declaration names.
pub(crate) fn explain(src: Src, node: Node) -> Option<ContractItem> {
    let name = node.child_by_field_name("name")?;
    let written = src.norm(name);
    let plain = written.strip_prefix("r#")?;
    Some(ContractItem {
        group: "Identifier".into(),
        text: format!("{written} is a raw identifier: r# tells Rust to read {plain} as a name even when it is a keyword. The prefix is syntax, not part of the name; this declaration names {plain}."),
        span: src.span(name),
        type_info: None,
    })
}
