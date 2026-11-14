//! Explain an entire match-arm wildcard without confusing nested patterns or bindings.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::children;
use tree_sitter::Node;

/// Return the enclosing match and the `_` token for an entire wildcard arm.
pub(crate) fn explain<'t>(node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "match_arm" || node.has_error() { return None; }
    let pattern = node.child_by_field_name("pattern")?;
    // Only `_` directly inside match_pattern is an entire wildcard. A nested
    // wildcard such as Some(_) or a binding such as _name does not qualify.
    let wildcard = children(pattern).into_iter().find(|child| child.kind() == "_" && !child.is_missing())?;
    let guard = pattern.child_by_field_name("condition");
    let enclosing = node.parent()?.parent()?;
    if enclosing.kind() != "match_expression" { return None; }
    let guard_text = if guard.is_some() {
        " This arm has a guard: even though _ matches, the guard must also be true; if it is false, Rust continues with later arms. A guarded wildcard alone does not cover every case for the compiler's exhaustiveness check."
    } else {
        " An unguarded _ covers every remaining value and can make a match exhaustive. Rust checks that all possible values are covered; an earlier unguarded wildcard also makes later arms unreachable."
    };
    Some((enclosing, wildcard, ReadingContent::Detail {
        label: "Match any remaining value".into(),
        text: format!("Arms are considered in order. The wildcard _ matches any value reaching this arm, so when earlier arms do not select it, the expression after => is evaluated. _ does not introduce a variable or bind the matched value.{}", guard_text),
        definition: None,
        borrow: false,
    }))
}

