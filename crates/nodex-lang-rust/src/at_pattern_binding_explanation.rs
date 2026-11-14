//! Explain `name @ pattern` without assuming any path's resolved identity.
use nodex_ir::{Span, reading::ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn captured<'t>(root: Node<'t>, skip: Option<Node<'t>>) -> Vec<Node<'t>> {
    let mut pending = vec![root];
    let mut found = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == skip || node.is_extra() || node.kind() == "token_tree" { continue; }
        if node.kind() == "captured_pattern" { found.push(node); }
        pending.extend(named_children(node).into_iter().rev());
    }
    found
}

/// Explain the pattern in a match arm or local/conditional binding.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Span, Node<'t>, ReadingContent)> {
    let (anchor, roots): (Span, Vec<(Node<'t>, Option<Node<'t>>)>) = match node.kind() {
        "match_expression" => {
            let Some(body) = node.child_by_field_name("body") else { return Vec::new(); };
            let roots = named_children(body).into_iter().filter(|arm| arm.kind() == "match_arm")
                .filter_map(|arm| arm.child_by_field_name("pattern"))
                .map(|pattern| (pattern, pattern.child_by_field_name("condition"))).collect();
            (src.header_span(node, Some(body)), roots)
        }
        "let_condition" | "let_declaration" => {
            let Some(pattern) = node.child_by_field_name("pattern") else { return Vec::new(); };
            (src.span(node), vec![(pattern, None)])
        }
        _ => return Vec::new(),
    };
    roots.into_iter().flat_map(|(root, skip)| captured(root, skip)).filter_map(|pattern| {
        let parts: Vec<_> = named_children(pattern).into_iter().filter(|part| !part.is_extra()).collect();
        let [binding, checked] = parts.as_slice() else { return None; };
        let at = children(pattern).into_iter().find(|child| child.kind() == "@")?;
        Some((anchor.clone(), at, ReadingContent::Detail {
            label: "Bind the value while checking a pattern".into(),
            text: format!("In {} @ {}, the part after `@` must match. If it succeeds, {} names the entire value matched at this position, not just a field inside it. The subpattern may also introduce its own bindings; `_` ignores the part it covers. Binding by value can move or copy according to the type; `ref` or `ref mut` can instead borrow. The names used inside the subpattern, such as `Some`, are resolved in context; their spelling alone does not identify a particular enum.", src.norm(*binding), src.norm(*checked), src.norm(*binding)),
            definition: None, borrow: false,
        }))
    }).collect()
}
