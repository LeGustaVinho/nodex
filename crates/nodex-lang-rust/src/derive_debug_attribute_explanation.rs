//! Explain `#[derive(Debug)]` without assuming which derive macro resolves.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "struct_item" | "enum_item" | "union_item") { return Vec::new(); }
    let Some(type_name) = declaration.child_by_field_name("name") else { return Vec::new() };
    let mut previous = declaration.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(item) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = item.prev_named_sibling();
        if item.kind() != "attribute_item" || item.has_error() { continue; }
        let Some(attr) = named_children(item).into_iter().find(|part| part.kind() == "attribute") else { continue };
        let Some(name) = named_children(attr).into_iter().find(|part| part.kind() == "identifier") else { continue };
        if src.text(name) != "derive" { continue; }
        let Some(args) = attr.child_by_field_name("arguments") else { continue };
        let tokens: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
        if tokens.len() < 3 || tokens.first().is_none_or(|token| token.kind() != "(") || tokens.last().is_none_or(|token| token.kind() != ")") { continue; }
        for path in tokens[1..tokens.len()-1].split(|token| token.kind() == ",") {
            let Some(last) = path.last() else { continue };
            if !matches!(last.kind(), "identifier" | "type_identifier") || src.text(*last) != "Debug" { continue; }
            // Accept only a path composed of identifiers and ::; never treat tokens
            // inside a string or nested group as an invocation of this derive.
            if path.iter().any(|part| !matches!(part.kind(), "identifier" | "type_identifier" | "::")) { continue; }
            let union_note = if declaration.kind() == "union_item" { " The standard Debug derive cannot be derived for a union, so that case requires a different implementation or derive." } else { "" };
            result.push(ContractItem {
                group: "Derive Debug".into(),
                text: format!("This #[derive(...)] attribute asks the compiler to run the listed derive macro for {}. If Debug resolves to Rust's standard derive and expansion succeeds, it generates an implementation of the Debug formatting trait for this type. That implementation can be used by formats such as {{:?}} to produce developer-oriented text; its exact output is not a stable serialization format. Field types and generic parameters can impose requirements. The attribute does not itself print anything; formatting happens only when resulting code requests it. A custom derive can share this name, so the written spelling alone does not prove which implementation is generated.{}", src.norm(type_name), union_note),
                span: src.span(*last), type_info: None,
            });
        }
    }
    result
}
