//! Explain list-form `vec!` without guessing which macro the name resolves to.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let name = node.child_by_field_name("macro")?;
    if src.norm(name) != "vec" { return None; }
    let tree = named_children(node).into_iter().find(|child| child.kind() == "token_tree")?;
    if tree.has_error() || !src.text(tree).starts_with('[') { return None; }
    if children(tree).iter().any(|part| part.kind() == ";") { return None; }
    let bang = children(node).into_iter().find(|part| part.kind() == "!")?;
    Some((bang, ReadingContent::Detail {
        label: "Vector list macro".into(),
        text: "The `!` invokes a macro, which expands the tokens inside the brackets during compilation. If this `vec!` resolves to Rust's standard macro, the comma-separated expressions provide elements for a `Vec`, an owned growable collection, in the order written; the expanded code constructs the vector when it runs. The vector allocates storage when needed, but an empty vector or one holding zero-sized elements need not allocate. This name can refer to another macro, so its spelling alone does not establish that standard behavior. The separate `vec![value; count]` form has different repetition semantics.".into(),
        definition: None,
        borrow: false,
    }))
}
