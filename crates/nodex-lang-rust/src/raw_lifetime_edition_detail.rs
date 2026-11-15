//! Explain the edition rule for raw lifetime and loop-label spellings.
//! Rust Reference: https://doc.rust-lang.org/reference/tokens.html#lifetimes-and-loop-labels
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn raw_name<'a>(src: Src<'a>, node: Node<'_>) -> Option<&'a str> {
    let text = src.text(node).trim_end_matches(':');
    let name = text.strip_prefix("'r#")?;
    if name.is_empty() || matches!(name, "_" | "crate" | "self" | "Self" | "super") { return None; }
    Some(name)
}

pub(crate) fn contract(src: Src<'_>, item: Node<'_>) -> Vec<ContractItem> {
    if !matches!(item.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "trait_item" | "type_item" | "associated_type" | "field_declaration") { return Vec::new(); }
    let body = item.child_by_field_name("body");
    let mut stack = vec![item];
    let mut output = Vec::new();
    while let Some(node) = stack.pop() {
        if Some(node) == body { continue; }
        if node.kind() == "lifetime" {
            if let Some(name) = raw_name(src, node) {
                output.push(ContractItem {
                    group: "Raw lifetime edition".into(),
                    text: format!("`'r#{name}` is a raw lifetime spelling: `r#` allows the lifetime name {name} to be a keyword and is not part of that name. Rust 2021 and later accept this spelling as one lifetime token; earlier editions tokenize it as `'r`, `#`, and `{name}` instead. The lifetime describes a validity relationship rather than a runtime value. The spelling alone does not establish which edition this crate uses."),
                    span: src.span(node), type_info: None,
                });
            }
        } else { stack.extend(named_children(node).into_iter().rev()); }
    }
    output
}

pub(crate) fn label<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Span, ReadingContent)> {
    if node.kind() != "label" { return None; }
    let name = raw_name(src, node)?;
    let owner = node.parent().filter(|parent| matches!(parent.kind(), "loop_expression" | "while_expression" | "for_expression" | "break_expression" | "continue_expression"))?;
    let prefix = src.index.span(src.file, node.start_byte() + 1, node.start_byte() + 3);
    Some((owner, prefix, ReadingContent::Detail {
        label: "Raw label and edition".into(),
        text: format!("`'r#{name}` is a raw loop-label spelling, allowing the label name {name} to be a keyword; `r#` is not part of the name. In Rust 2021 and later it is one lifetime-or-label token. In earlier editions the same characters are tokenized as `'r`, `#`, and `{name}`. Here it names a control-flow target, not a lifetime or runtime value. This syntax alone does not establish the crate's edition."),
        definition: None, borrow: false,
    }))
}
