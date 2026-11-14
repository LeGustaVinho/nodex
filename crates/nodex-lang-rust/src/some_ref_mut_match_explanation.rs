//! Explain `Some(ref mut x)` when it appears in a match arm.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "tuple_struct_pattern" || node.has_error() { return None; }
    let path = node.child_by_field_name("type")?;
    let name = src.norm(path);
    if name != "Some" && !name.ends_with("::Some") { return None; }
    let parts: Vec<_> = named_children(node).into_iter().filter(|part| *part != path && !part.is_extra()).collect();
    let [binding] = parts.as_slice() else { return None; };
    if binding.kind() != "ref_pattern" || !named_children(*binding).iter().any(|part| part.kind() == "mut_pattern") { return None; }
    let keyword = children(*binding).into_iter().find(|part| part.kind() == "ref")?;
    let mut current = node;
    let arm = loop {
        current = current.parent()?;
        if current.kind() == "match_arm" { break current; }
        if matches!(current.kind(), "block" | "match_expression") { return None; }
    };
    let pattern = arm.child_by_field_name("pattern")?;
    if node.start_byte() < pattern.start_byte() || node.end_byte() > pattern.end_byte() { return None; }
    let owner = arm.parent()?.parent().filter(|parent| parent.kind() == "match_expression")?;
    Some((owner, keyword, ReadingContent::Detail {
        label: "Match and mutably borrow the inner value".into(),
        text: format!("The pattern {} first checks the tuple-style form named {name}. If it matches, `ref mut` binds its contained part through an exclusive mutable reference instead of moving that part; use `*` on the binding to access or change it. The borrow requires mutable access to that part and restricts conflicting access while active. If {name} resolves to the standard Option::Some variant, it matches an option containing a value; the written name alone does not prove that resolution. The binding is available on the successful arm path.", src.norm(node)),
        definition: None,
        borrow: false,
    }))
}
