//! Explain the edition-sensitive `try` keyword and related spellings.
//! Rust Reference: https://doc.rust-lang.org/reference/keywords.html#reserved-keywords
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

const RAW_TEXT: &str = "`r#try` is a raw identifier whose name is `try`; `r#` allows the name where the word would otherwise be a reserved keyword. `try` became reserved in the Rust 2018 edition and was not a keyword in Rust 2015. This identifier is not the experimental `try { ... }` construct and its spelling alone does not establish what item it resolves to.";

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    let name = node.child_by_field_name("name")?;
    if src.text(name) != "r#try" { return None; }
    Some(ContractItem { group: "Raw identifier try".into(), text: RAW_TEXT.into(), span: src.span(name), type_info: None })
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (label, text, span) = match node.kind() {
        "try_block" => {
            let keyword = children(node).into_iter().find(|child| child.kind() == "try")?;
            ("Experimental try block", "In Rust 2018 and later, `try` is a reserved keyword; in Rust 2015 it was not a keyword. `try { ... }` is an experimental try block, requiring a compiler and feature configuration that supports `try_blocks`. It creates a scope for `?`: a propagated residual finishes the try block instead of necessarily returning from the enclosing function. It is not C#-style try/catch and does not catch panics. The source alone does not establish that the current build enables this feature.", src.span(keyword))
        }
        "macro_invocation" => {
            let name = node.child_by_field_name("macro")?;
            if !matches!(src.text(name), "try" | "r#try") { return None; }
            let bang = children(node).into_iter().find(|part| part.kind() == "!")?;
            ("Legacy try macro spelling", "This is a macro invocation, not a `try { ... }` block. `try` was not a keyword in Rust 2015, but is reserved from Rust 2018 onward; a macro with this name is written `r#try!` in later editions. If it resolves to Rust's deprecated standard `try!` macro, it unwraps an Ok result or returns an error from the enclosing function; the `?` operator is its usual replacement. Another macro may have the same name, so the spelling alone does not establish that behavior.", src.span(bang))
        }
        "identifier" if src.text(node) == "r#try" => {
            if node.parent().is_some_and(|parent| parent.kind() == "macro_invocation" || parent.child_by_field_name("name") == Some(node)) { return None; }
            let prefix = src.index.span(src.file, node.start_byte(), node.start_byte() + 2);
            ("Raw identifier try", RAW_TEXT, prefix)
        }
        _ => return None,
    };
    Some((span, ReadingContent::Detail { label: label.into(), text: text.into(), definition: None, borrow: false }))
}
