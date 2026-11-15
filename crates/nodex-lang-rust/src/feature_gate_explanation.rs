//! Explain written crate-level unstable feature gates.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "source_file" { return Vec::new(); }
    named_children(node).into_iter().filter_map(|item| {
        if item.kind() != "inner_attribute_item" || item.has_error() { return None; }
        let attribute = named_children(item).into_iter().find(|part| part.kind() == "attribute")?;
        let parts = named_children(attribute);
        if src.text(*parts.first()?) != "feature" { return None; }
        let tree = parts.into_iter().find(|part| part.kind() == "token_tree")?;
        let tokens: Vec<_> = children(tree).into_iter().filter(|part| !part.is_extra()).collect();
        if tokens.len() < 3 || tokens[0].kind() != "(" || tokens.last()?.kind() != ")" { return None; }
        let mut names = Vec::new();
        let mut expect_name = true;
        for token in &tokens[1..tokens.len()-1] {
            if expect_name {
                if token.kind() != "identifier" { return None; }
                names.push(src.norm(*token));
            } else if token.kind() != "," { return None; }
            expect_name = !expect_name;
        }
        if names.is_empty() { return None; }
        Some(ContractItem {
            group: "Unstable feature gates".into(),
            text: format!("This inner crate attribute requests the Rust compiler feature gates: {}. These names identify experimental compiler or language features that the crate wants to use; the attribute does not itself contain or run the feature's code. Unstable features generally require a nightly compiler and can change or be removed. Whether a gate exists, remains unstable, is allowed by the toolchain, and is active in this build depends on the compiler and configuration; this source alone does not establish those facts. The attribute is intended for the crate root, whose identity must also be established separately.", names.join(", ")),
            span: src.span(item), type_info: None,
        })
    }).collect()
}
