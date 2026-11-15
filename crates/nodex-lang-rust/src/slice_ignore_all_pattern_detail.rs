//! Explain `let [..] = value` without inventing bindings or element moves.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" || node.has_error() { return None; }
    let pattern = node.child_by_field_name("pattern")?;
    if pattern.kind() != "slice_pattern" { return None; }
    let parts: Vec<_> = named_children(pattern).into_iter().filter(|part| !part.is_extra()).collect();
    let [rest] = parts.as_slice() else { return None; };
    if rest.kind() != "remaining_field_pattern" || src.text(*rest) != ".." { return None; }
    let value = node.child_by_field_name("value")?;
    Some((*rest, ReadingContent::Detail {
        label: "Ignore every element in a sequence pattern".into(),
        text: format!("Evaluate {} as the right-hand expression, then match it with [..]. Here .. is a rest pattern: it accepts and ignores all elements, including when there are zero, and creates no element bindings. For a compatible array or slice, this particular pattern matches every length, so it is irrefutable and does not need an else branch. The pattern is not indexing or a range value. The pattern itself does not read or move individual ignored elements, but evaluating the right-hand expression can still have effects or ownership consequences; its type must be compatible with the pattern.", src.norm(value)),
        definition: None,
        borrow: false,
    }))
}
