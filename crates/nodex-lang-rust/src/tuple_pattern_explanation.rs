//! Explain tuple patterns in match arms, bindings, and function parameters.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn tuple_patterns<'t>(root: Node<'t>, skip: Option<Node<'t>>) -> Vec<Node<'t>> {
    let mut pending = vec![root];
    let mut found = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == skip || node.is_extra() || node.kind() == "token_tree" { continue; }
        if node.kind() == "tuple_pattern" && children(node).iter().any(|part| part.kind() == ",") { found.push(node); }
        pending.extend(named_children(node).into_iter().rev());
    }
    found
}

fn description(src: Src<'_>, pattern: Node<'_>) -> String {
    format!("{} is a tuple pattern: each position checks or binds the corresponding tuple element in order. For example, `(x, y)` binds the first element to x and the second to y; `_` ignores a position, and `..` can stand for remaining positions. Each element must satisfy its subpattern. Bindings by value can copy or move according to type, while reference patterns can borrow. Parentheses alone can group a pattern; a one-element tuple pattern requires a comma.", src.norm(pattern))
}

/// Explain tuple patterns on function parameters, whose symbol owns the header.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let Some(parameters) = node.child_by_field_name("parameters") else { return Vec::new(); };
    named_children(parameters).into_iter().filter_map(|param| param.child_by_field_name("pattern"))
        .flat_map(|pattern| tuple_patterns(pattern, None)).map(|pattern| ContractItem {
            group: "Tuple pattern".into(), text: description(src, pattern), span: src.span(pattern), type_info: None,
        }).collect()
}

/// Explain patterns attached to a visible match or local/conditional binding.
pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Span, Node<'t>, ReadingContent)> {
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
    roots.into_iter().flat_map(|(root, skip)| tuple_patterns(root, skip)).filter_map(|pattern| {
        // Direct tuple let already has a dedicated, source-linked explanation.
        if node.kind() == "let_declaration" && node.child_by_field_name("pattern") == Some(pattern) { return None; }
        let open = children(pattern).into_iter().find(|part| part.kind() == "(")?;
        Some((anchor.clone(), open, ReadingContent::Detail {
            label: "Match tuple positions".into(), text: description(src, pattern), definition: None, borrow: false,
        }))
    }).collect()
}
