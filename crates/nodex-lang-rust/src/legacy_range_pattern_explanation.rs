//! Explain the pre-2021 inclusive range pattern written with three dots.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "match_expression" { return Vec::new(); }
    let Some(body) = node.child_by_field_name("body") else { return Vec::new(); };
    let Some(anchor) = children(node).into_iter().find(|part| part.kind() == "match") else { return Vec::new(); };
    let mut result = Vec::new();
    for arm in named_children(body).into_iter().filter(|part| part.kind() == "match_arm") {
        let Some(pattern) = arm.child_by_field_name("pattern") else { continue; };
        let mut stack = vec![pattern];
        while let Some(part) = stack.pop() {
            if part.kind() == "range_pattern" {
                if let Some(operator) = children(part).into_iter().find(|child| src.text(*child) == "...") {
                    result.push((anchor, operator, ReadingContent::Detail {
                        label: "Legacy inclusive range pattern".into(),
                        text: format!("`{}` is a legacy range pattern: it matches values from its lower bound through its upper bound, including both ends. For example, `1...5` matches 1, 2, 3, 4, or 5; the wildcard `_` in a later arm can handle other values. The `...` spelling is deprecated and normally warns in Rust 2015 and 2018, and is an error in Rust 2021 and later. Write `..=` instead (for example `1..=5`); this preserves the inclusive meaning. The parser's ability to display this pattern does not establish the project's edition or successful compilation.", src.norm(part)),
                        definition: None, borrow: false,
                    }));
                }
                continue;
            }
            stack.extend(named_children(part));
        }
    }
    result
}
