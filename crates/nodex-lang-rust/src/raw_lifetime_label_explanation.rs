//! Explain raw lifetime and label names in their respective contexts.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn text(src: Src<'_>, node: Node<'_>) -> Option<String> {
    let written = src.norm(node);
    let plain = written.strip_prefix("'r#")?.trim_end_matches(':');
    if plain.is_empty() { return None; }
    match node.kind() {
        "lifetime" => Some(format!("{written} is a raw lifetime name. The apostrophe marks a lifetime and r# allows a keyword to be used as its name; the prefix is syntax, not part of the name. A lifetime describes how long a reference must remain valid, not a runtime value or a timer.")),
        "label" => Some(format!("{written} is a raw label name. The apostrophe marks a label and r# allows a keyword to be used as its name; the prefix is syntax, not part of the name. A matching break label selects the labeled loop or block, while continue selects a loop; the label is not a lifetime in this context.")),
        _ => None,
    }
}

/// Add lifetime explanations to a declaration's existing visible contract.
pub(crate) fn contract(src: Src<'_>, item: Node<'_>) -> Vec<ContractItem> {
    if !matches!(item.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "trait_item" | "type_item") { return Vec::new(); }
    let body = item.child_by_field_name("body");
    let mut stack = vec![item];
    let mut result = Vec::new();
    while let Some(node) = stack.pop() {
        if Some(node) == body { continue; }
        if node.kind() == "lifetime" {
            if let Some(explanation) = text(src, node) {
                result.push(ContractItem { group: "Raw lifetime".into(), text: explanation, span: src.span(node), type_info: None });
            }
        } else {
            stack.extend(named_children(node).into_iter().rev());
        }
    }
    result
}

/// Attach label explanations to flow readings when visiting a label node.
pub(crate) fn label(src: Src<'_>, node: Node<'_>) -> Option<ReadingContent> {
    if node.kind() != "label" { return None; }
    Some(ReadingContent::Detail { label: "Raw label".into(), text: text(src, node)?, definition: None, borrow: false })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn separates_raw_lifetimes_from_raw_loop_labels() {
        let source = "fn f<'r#type>(x: &'r#type str) { 'r#loop: loop { break 'r#loop; } }";
        let tree = crate::parse(source).unwrap();
        let index = LineIndex::new(source);
        let src = Src { text: source, file: "test.rs", index: &index };
        let function = named_children(tree.root_node())[0];
        let lifetimes = contract(src, function);
        assert_eq!(lifetimes.len(), 2);
        assert!(lifetimes[0].text.contains("raw lifetime name"));
        assert!(lifetimes.iter().all(|item| !item.text.contains("raw label name")));
        let mut stack = vec![function];
        let mut labels = Vec::new();
        while let Some(node) = stack.pop() {
            if let Some(detail) = label(src, node) { labels.push(detail); }
            stack.extend(named_children(node));
        }
        assert!(!labels.is_empty());
        assert!(labels.iter().all(|detail| matches!(detail, ReadingContent::Detail { text, .. } if text.contains("not a lifetime"))));
    }
}

