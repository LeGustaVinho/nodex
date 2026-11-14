//! Explain a two-element sequence pattern in an ordinary `let` declaration.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, declaration: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if declaration.kind() != "let_declaration" || declaration.has_error()
        || declaration.child_by_field_name("alternative").is_some()
        || declaration.child_by_field_name("value").is_none() { return None; }
    let pattern = declaration.child_by_field_name("pattern")?;
    if pattern.kind() != "slice_pattern" { return None; }
    let parts: Vec<_> = named_children(pattern).into_iter().filter(|part| !part.is_extra()).collect();
    if parts.len() != 2 || !parts.iter().all(|part| part.kind() == "identifier") { return None; }
    let bracket = children(pattern).into_iter().find(|child| child.kind() == "[" && !child.is_missing())?;
    let first = src.norm(parts[0]);
    let second = src.norm(parts[1]);
    Some((bracket, ReadingContent::Detail {
        label: "Bind two array positions".into(),
        text: format!("Evaluate the expression on the right and match its value against [{first}, {second}]. If these names introduce bindings, {first} receives position 0 and {second} receives position 1; matching can copy, move, or borrow elements depending on their types and pattern. An ordinary let requires an irrefutable pattern: it must match every value of the initializer's type. A fixed-size array of exactly two elements satisfies this shape, while a slice of unknown length can have a different number of elements and this exact-length pattern is not irrefutable for it. The compiler checks the actual type and name resolution; the pattern alone does not prove them."),
        definition: None,
        borrow: false,
    }))
}
