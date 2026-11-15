//! Explain the trailing where clause of a generic associated type implementation.
//! Rust Reference: https://doc.rust-lang.org/reference/items/type-aliases.html
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "type_item" || node.has_error() { return None; }
    let implementation = node.parent()?.parent()?;
    if implementation.kind() != "impl_item" { return None; }
    let trait_name = implementation.child_by_field_name("trait")?;
    let target = implementation.child_by_field_name("type")?;
    let name = node.child_by_field_name("name")?;
    let params = node.child_by_field_name("type_parameters")?;
    let chosen = node.child_by_field_name("type")?;
    let clause = named_children(node).into_iter().find(|part| part.kind() == "where_clause" && part.start_byte() >= chosen.end_byte())?;
    let clone_note = if named_children(clause).into_iter().any(|predicate| {
        predicate.child_by_field_name("bounds").is_some_and(|bounds| src.norm(bounds).trim_start_matches(':').trim() == "Clone")
    }) { " If Clone resolves to Rust's standard trait, that requirement concerns explicit cloning support; it does not automatically clone values or guarantee independent deep copies." } else { "" };
    let vec_note = if chosen.kind() == "generic_type" && chosen.child_by_field_name("type").is_some_and(|name| src.norm(name) == "Vec") {
        " If Vec resolves to the standard Vec type, it denotes a growable owned collection of its element type."
    } else { "" };
    Some(ContractItem {
        group: "Generic associated type trailing where".into(),
        text: format!("In `impl {} for {}`, `type {}{} = {}` defines the associated type family {} for this implementation; the arguments in {} parameterize that family. The following `{}` is a where clause: it states requirements that apply to the type definition, rather than being part of the right-hand type. For an associated type in a trait implementation, putting this clause after the assigned type is the preferred position; the older position before `=` is deprecated. Rust checks that the implementation and its requirements are compatible with the trait; the spelling alone does not prove this code compiles or resolve the names.{}{}", src.norm(trait_name), src.norm(target), src.norm(name), src.norm(params), src.norm(chosen), src.norm(name), src.norm(params), src.norm(clause), clone_note, vec_note),
        span: src.span(clause), type_info: None,
    })
}
