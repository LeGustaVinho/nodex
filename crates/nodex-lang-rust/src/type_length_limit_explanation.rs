//! Explain the crate's written compile-time type complexity limit.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn numeric_literal(src: Src<'_>, value: Node<'_>) -> Option<String> {
    let text = src.text(value);
    let body = match value.kind() {
        "string_literal" => text.strip_prefix('"')?.strip_suffix('"')?,
        "raw_string_literal" => {
            let rest = text.strip_prefix('r')?;
            let hashes = rest.chars().take_while(|ch| *ch == '#').count();
            rest.get(hashes..)?.strip_prefix('"')?.strip_suffix(&format!("\"{}", "#".repeat(hashes)))?
        }
        _ => return None,
    };
    if body.is_empty() || !body.chars().all(|ch| ch.is_ascii_digit()) { return None; }
    Some(body.to_owned())
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "source_file" { return Vec::new(); }
    named_children(node).into_iter().filter_map(|part| {
        if part.kind() != "inner_attribute_item" || part.has_error() { return None; }
        let attribute = named_children(part).into_iter().find(|child| child.kind() == "attribute")?;
        let path = named_children(attribute).into_iter().next()?;
        if src.text(path) != "type_length_limit" { return None; }
        let limit = numeric_literal(src, attribute.child_by_field_name("value")?)?;
        Some(ContractItem {
            group: "Type length limit".into(),
            text: format!("This crate-root attribute specifies {limit} as the maximum number of type substitutions when the compiler constructs a concrete type during monomorphization (turning generic code into code for particular types). It concerns compilation complexity, not the size of a value in memory. Currently rustc enforces this limit only when the nightly `-Zenforce-type-length-limit` flag is active; this source alone does not prove that enforcement is active. Only the first occurrence on an item takes effect; later occurrences are linted."),
            span: src.span(part), type_info: None,
        })
    }).collect()
}
