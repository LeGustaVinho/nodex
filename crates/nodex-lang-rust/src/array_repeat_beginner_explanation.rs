//! Explain the two operands and corner cases of an array repeat expression.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    let (value, _, length) = nodex_lang_api::syntax_sites::rust_array_repeat(node)?;
    let open = children(node).into_iter().find(|part| part.kind() == "[")?;
    Some((open, ReadingContent::Detail {
        label: "Understand array repetition".into(),
        text: format!("In [{}; {}], the expression before `;` is the repeat operand and the expression after it is the array length. The length must be a compile-time `usize` constant or an inferred constant; it determines the fixed array type. For an ordinary operand, Rust evaluates it once even when the length is zero; then it is copied for the elements, or its result is discarded for an empty array. Repeating an ordinary value more than once requires `Copy`, and does not call `Clone`. A const block or a path to a constant item is the exception: it is evaluated once per element, is not evaluated when the length is zero, and need not be `Copy` for repetition. If the operand is a name, its resolution decides whether it is a constant item or an ordinary value.", src.norm(value), src.norm(length)),
        definition: None,
        borrow: false,
    }))
}
