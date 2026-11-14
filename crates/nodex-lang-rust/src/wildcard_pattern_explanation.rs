//! Explain `_` inside a destructuring pattern without confusing type placeholders.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::children;
use tree_sitter::Node;

/// Match wildcard tokens only inside pattern fields. Entire `let _` and match-arm
/// `_` already have dedicated explanations; this covers nested wildcards.
pub(crate) fn explain<'t>(node: Node<'t>) -> Vec<(Node<'t>, Node<'t>, ReadingContent)> {
    if !matches!(node.kind(), "let_declaration" | "let_condition" | "match_arm" | "for_expression") || node.has_error() { return Vec::new(); }
    let Some(pattern) = node.child_by_field_name("pattern") else { return Vec::new(); };
    if pattern.kind() == "_" { return Vec::new(); }
    let guard = pattern.child_by_field_name("condition");
    let mut todo = vec![pattern];
    let mut found = Vec::new();
    while let Some(part) = todo.pop() {
        if Some(part) == guard || part.kind() == "token_tree" { continue; }
        if part.kind() == "_" {
            // A top-level match arm wildcard has a dedicated arm explanation.
            if node.kind() == "match_arm" && part.parent() == Some(pattern) { continue; }
            let anchor = if node.kind() == "match_arm" {
                node.parent().and_then(|parent| parent.parent())
                    .and_then(|owner| children(owner).into_iter().find(|child| child.kind() == "match"))
                    .unwrap_or(part)
            } else { part };
            found.push((anchor, part, ReadingContent::Detail {
                label: "Ignore this part of the pattern".into(),
                text: "The wildcard pattern `_` accepts any value in this position without creating a variable for it. This wildcard itself does not bind, copy, move, or borrow the ignored part; surrounding patterns can still test the value's shape and bind other parts. The matched expression is still evaluated as required by its context. Unlike `_`, a name such as `_item` is a binding that can move, copy, or borrow a value.".into(),
                definition: None, borrow: false,
            }));
            continue;
        }
        todo.extend(children(part).into_iter().rev());
    }
    found
}
