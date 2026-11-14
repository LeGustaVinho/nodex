//! Explain creating a read-oriented raw pointer without creating a reference.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "reference_expression" || node.has_error() { return None; }
    let parts = children(node);
    let raw = parts.iter().copied().find(|part| part.kind() == "raw")?;
    if !parts.iter().any(|part| part.kind() == "const") { return None; }
    let place = node.child_by_field_name("value")?;
    Some((raw, ReadingContent::Detail {
        label: "Create a raw const pointer".into(),
        text: format!("`&raw const {}` creates a `*const` raw pointer to the place's address without first creating a Rust reference or reading the value stored there. `const` means this pointer does not permit writing through it directly; it does not make the underlying storage immutable. Unlike a reference, the pointer is not guaranteed to stay valid or aligned and does not keep its target alive. Creating the pointer does not by itself require dereferencing it, but evaluating the place may have its own safety requirements. Dereferencing the pointer requires unsafe code and the address must satisfy the applicable validity, alignment, initialization, and aliasing rules. Casting to `*mut` alone does not make a write valid.", src.norm(place)),
        definition: None,
        borrow: false,
    }))
}
