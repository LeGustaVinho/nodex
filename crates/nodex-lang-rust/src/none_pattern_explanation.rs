//! Explain the spelling `None` in a pattern without guessing its resolution.
use nodex_ir::{Span, reading::ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn collect<'t>(root: Node<'t>, skip: Option<Node<'t>>) -> Vec<Node<'t>> {
    let mut pending = vec![root];
    let mut sites = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == skip || node.is_extra() || matches!(node.kind(), "token_tree" | "scoped_identifier") { continue; }
        if node.kind() == "identifier" {
            sites.push(node);
            continue;
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    sites
}

/// Return one detail for each unqualified `None` occurring in a pattern.
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
    roots.into_iter().flat_map(|(pattern, guard)| collect(pattern, guard)).filter(|site| src.text(*site) == "None")
        .map(|site| (anchor.clone(), site, ReadingContent::Detail {
            label: "Understand a `None` pattern".into(),
            text: "`None` is a name in this pattern. If it resolves to the standard `Option::None` variant, the pattern matches an optional value with no contained value; there is no payload to extract. A different constant or unit variant named `None` would match that item instead. If the name is treated as a new binding, it instead matches any value and binds that name. The spelling alone does not establish which case applies; name resolution and context decide.".into(),
            definition: None, borrow: false,
        })).collect()
}
