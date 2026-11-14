//! Explain Rust's `for pattern in expression` loop to a reader new to Rust.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "for_expression" || node.has_error() { return None; }
    let keyword = children(node).into_iter().find(|part| part.kind() == "for" && !part.is_missing())?;
    let pattern = node.child_by_field_name("pattern")?;
    let value = node.child_by_field_name("value")?;
    node.child_by_field_name("body")?;
    Some((keyword, ReadingContent::Detail {
        label: "Repeat for each iterator item".into(),
        text: format!("Evaluate {} once and obtain an iterator through Rust's IntoIterator mechanism. Repeatedly request the next item; for each item, match it against the pattern {} and run the body with its bindings. When no next item remains, execution continues after the loop. A break can finish the loop early and continue skips to the next iteration. Whether the source is moved, copied, or borrowed, and what each item contains, depends on the source expression and the applicable iterator implementation; the syntax alone does not establish these details.", src.norm(value), src.norm(pattern)),
        definition: None,
        borrow: false,
    }))
}
