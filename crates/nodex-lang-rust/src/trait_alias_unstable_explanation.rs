//! Explain a simple unstable trait-alias declaration recovered as a parse error.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// The grammar currently represents this spelling as ERROR + an empty
/// statement. Inspect only direct module children and a narrowly verified token
/// sequence; preserve the diagnostic instead of claiming it parsed normally.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    let mut items = Vec::new();
    for error in children(container).into_iter().filter(|part| part.is_error()) {
        let parts: Vec<_> = children(error).into_iter().filter(|part| !part.is_extra()).collect();
        let [keyword, alias, equals, first, plus, second] = parts.as_slice() else { continue; };
        if keyword.kind() != "trait" || alias.kind() != "identifier" || equals.kind() != "="
            || first.kind() != "identifier" || plus.kind() != "+" || second.kind() != "identifier" { continue; }
        let mut after = error.next_named_sibling();
        while after.is_some_and(|part| part.is_extra()) { after = after.and_then(|part| part.next_named_sibling()); }
        let Some(end) = after.filter(|part| part.kind() == "empty_statement" && src.text(*part) == ";") else { continue; };
        items.push(ContractItem {
            group: "Unstable trait alias".into(),
            text: format!("trait {} = {} + {}; proposes a trait alias named {} for the listed trait bounds. Where accepted, using the alias as a bound expresses the combined requirements; it does not declare a new trait with its own methods or create a separate trait implementation. Trait aliases are unstable and require a suitable nightly compiler with #![feature(trait_alias)]; stable Rust does not accept this feature. Current implementations limit which trait combinations are accepted, and the names alone do not establish whether this particular combination is valid. The parser recovered this spelling as an error, so the diagnostic should not be interpreted as proof that the alias was compiled.", src.norm(*alias), src.norm(*first), src.norm(*second), src.norm(*alias)),
            span: src.index.span(src.file, error.start_byte(), end.end_byte()), type_info: None,
        });
    }
    items
}
