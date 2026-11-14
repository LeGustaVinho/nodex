//! Explain a bounded, end-exclusive range used directly by a `for` loop.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "for_expression" || node.has_error() { return None; }
    let value = crate::syntax::unparen(node.child_by_field_name("value")?);
    if value.kind() != "range_expression" { return None; }
    let parts = children(value);
    let dots = parts.iter().find(|part| part.kind() == "..")?;
    let start = parts.iter().find(|part| part.is_named() && part.end_byte() <= dots.start_byte())?;
    let end = parts.iter().find(|part| part.is_named() && part.start_byte() >= dots.end_byte())?;
    let keyword = children(node).into_iter().find(|part| part.kind() == "in")?;
    Some((keyword, ReadingContent::Detail {
        label: "Iterate over a half-open range".into(),
        text: format!("Evaluate the bounds {} and {} when entering the loop and create the range {}. The range includes its start and excludes its end. For integer bounds, iteration produces consecutive integers starting at {} and stopping before {}; if the start is greater than or equal to the end, there are no iterations. `for` obtains an iterator and runs its body for each yielded value; a `break`, return, or panic can stop it earlier. The operand types and whether they support iteration must satisfy Rust's type rules.", src.norm(*start), src.norm(*end), src.norm(value), src.norm(*start), src.norm(*end)),
        definition: None,
        borrow: false,
    }))
}
