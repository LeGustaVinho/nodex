//! Explain floating-point literals with an explicit `f32` suffix.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "float_literal" || node.has_error() { return None; }
    let written = src.text(node);
    if !written.ends_with("f32") { return None; }
    Some((node, ReadingContent::Detail {
        label: "Floating-point literal with an f32 suffix".into(),
        text: format!("{} is a numeric literal. The suffix f32 explicitly gives it Rust's 32-bit floating-point type; it is part of the literal's syntax, not a variable or a function call. The decimal spelling is converted to a binary floating-point value: many decimal values need rounding because they have no exact binary32 representation. This literal denotes a single number, not a range or a string.", written),
        definition: None,
        borrow: false,
    }))
}
