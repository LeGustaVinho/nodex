//! Explain the edition-sensitive reserved `gen` keyword and experimental block.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

const IDENTIFIER_TEXT: &str = "`gen` became a reserved keyword in Rust 2024; in earlier editions it can be an ordinary identifier. `r#gen` is a raw identifier named `gen` and can be used where the keyword would otherwise prevent that name. The spelling does not establish the crate's edition or what this identifier resolves to.";

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    let name = node.child_by_field_name("name")?;
    if !matches!(src.text(name), "gen" | "r#gen") { return None; }
    Some(ContractItem { group: "Edition-sensitive gen identifier".into(), text: IDENTIFIER_TEXT.into(), span: src.span(name), type_info: None })
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (label, text, span) = match node.kind() {
        "gen_block" => {
            let keyword = children(node).into_iter().find(|child| src.text(*child) == "gen")?;
            ("Experimental gen block", "`gen` became reserved in Rust 2024; in earlier editions it can be an ordinary identifier. A `gen { ... }` block is experimental and requires a compiler and feature configuration supporting `gen_blocks`. It creates an iterator: its body can use `yield` to produce values one at a time as the iterator advances, suspending and resuming between values. The source alone does not establish that this feature is enabled or that the block compiles.", src.span(keyword))
        }
        "identifier" if matches!(src.text(node), "gen" | "r#gen") => {
            if node.parent().is_some_and(|parent| parent.child_by_field_name("name") == Some(node)) { return None; }
            ("Edition-sensitive gen identifier", IDENTIFIER_TEXT, src.span(node))
        }
        _ => return None,
    };
    Some((span, ReadingContent::Detail { label: label.into(), text: text.into(), definition: None, borrow: false }))
}
