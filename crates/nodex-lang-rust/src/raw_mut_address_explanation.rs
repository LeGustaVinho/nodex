//! Explain taking a mutable raw address without constructing a reference.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Return the raw keyword and explanation for `&raw mut place` expressions.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "reference_expression" { return None; }
    let parts = children(node);
    let raw = parts.iter().copied().find(|part| src.text(*part) == "raw")?;
    if !parts.iter().any(|part| part.kind() == "mutable_specifier" || src.text(*part) == "mut") { return None; }
    let place = node.child_by_field_name("value")?;
    let text = format!("&raw mut takes the address of {} and produces a mutable raw pointer (*mut T). It does not create a Rust reference, read the value, or modify the place merely by forming its address. A raw pointer does not keep the target alive or guarantee it is valid. Dereferencing it requires unsafe code; the programmer must ensure validity, alignment and applicable aliasing rules before reading or writing. Evaluating a complex place expression can itself have effects or require unsafe operations.", src.norm(place));
    Some((raw, ReadingContent::Detail { label: "Take a mutable raw address".into(), text, definition: None, borrow: false }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::{kit::LineIndex, syntax::named_children};

    #[test]
    fn distinguishes_raw_mut_from_raw_const_and_regular_borrows() {
        let text = "fn f(mut x: i32) { let a = &raw mut x; let b = &raw const x; let c = &mut x; }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let mut nodes = vec![tree.root_node()];
        let mut matches = Vec::new();
        while let Some(node) = nodes.pop() {
            if let Some((span, content)) = explain(src, node) { matches.push((src.text(span).to_string(), content)); }
            nodes.extend(named_children(node));
        }
        assert_eq!(matches.len(), 1, "{matches:?}");
        assert_eq!(matches[0].0, "raw");
        assert!(matches!(&matches[0].1, ReadingContent::Detail { text, .. } if text.contains("does not create a Rust reference") && text.contains("unsafe")));
    }
}
