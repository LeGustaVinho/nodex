//! Explain `Some(ref x)` without assuming that `Some` resolves to std::Option.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "tuple_struct_pattern" || node.has_error() { return None; }
    let constructor = node.child_by_field_name("type")?;
    let name = src.norm(constructor);
    if name.split("::").last()? != "Some" { return None; }
    let fields: Vec<_> = named_children(node).into_iter().filter(|part| *part != constructor).collect();
    if fields.len() != 1 || fields[0].kind() != "ref_pattern" { return None; }
    let reference = fields[0];
    if children(reference).iter().any(|part| matches!(part.kind(), "mutable_specifier" | "mut_pattern")) { return None; }
    let binding = named_children(reference).into_iter().find(|part| part.kind() == "identifier")?;
    let paren = children(node).into_iter().find(|part| part.kind() == "(" && !part.is_missing())?;
    let mut owner = node.parent()?;
    loop {
        if matches!(owner.kind(), "match_expression" | "if_expression" | "while_expression" | "for_expression" | "let_declaration") { break; }
        if matches!(owner.kind(), "function_item" | "closure_expression" | "source_file") { return None; }
        owner = owner.parent()?;
    }
    Some((owner, paren, ReadingContent::Detail {
        label: "Match a Some-shaped value and borrow its field".into(),
        text: format!("The pattern {} checks whether the value has the constructor or variant named {} and matches its single positional field. If it matches, ref {} binds a shared reference to that field rather than moving the field; the binding is available on the matching path. If {} resolves to the standard Option::Some variant, this is the case that contains a value, as opposed to None. The spelling alone does not prove which item Some refers to, and a failed match follows the surrounding construct's other path.", src.norm(node), name, src.norm(binding), name),
        definition: None,
        borrow: false,
    }))
}
