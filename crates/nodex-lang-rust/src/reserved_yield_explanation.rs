//! Explain reserved `yield`, raw identifiers, and unstable yield expressions.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

const RAW: &str = "`r#yield` is a raw identifier named `yield`; the `r#` prefix allows this reserved word to be used as a name where raw identifiers are permitted. It is not a yield expression, and its spelling does not determine what item it resolves to.";
const EXPR: &str = "`yield` is reserved in Rust. In an experimental coroutine, a yield expression suspends the coroutine and produces a value to its caller; when the coroutine is resumed, execution continues after the yield. This functionality requires unstable coroutine features and a compiler configuration that supports them; it is not ordinary stable Rust or an iterator return. This source alone does not establish that the build enables those features.";

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    let name = node.child_by_field_name("name")?;
    if src.text(name) != "r#yield" { return None; }
    Some(ContractItem { group: "Raw identifier yield".into(), text: RAW.into(), span: src.span(name), type_info: None })
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (label, text, span) = match node.kind() {
        "yield_expression" => {
            let keyword = children(node).into_iter().find(|part| part.kind() == "yield")?;
            ("Experimental yield expression", EXPR, src.span(keyword))
        }
        "identifier" if src.text(node) == "r#yield" => {
            if node.parent().is_some_and(|parent| parent.child_by_field_name("name") == Some(node)) { return None; }
            ("Raw identifier yield", RAW, src.index.span(src.file, node.start_byte(), node.start_byte() + 2))
        }
        _ => return None,
    };
    Some((span, ReadingContent::Detail { label: label.into(), text: text.into(), definition: None, borrow: false }))
}
