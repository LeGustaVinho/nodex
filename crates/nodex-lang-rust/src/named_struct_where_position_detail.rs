//! Explain where clauses before named-field struct bodies.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "struct_item" { return None; }
    let name = src.norm(node.child_by_field_name("name")?);
    let body = node.child_by_field_name("body")?;
    if body.kind() != "field_declaration_list" { return None; }
    let clause = named_children(node).into_iter().find(|part| part.kind() == "where_clause")?;
    if clause.start_byte() >= body.start_byte() { return None; }
    Some(ContractItem {
        group: "Where clause on named-field struct".into(),
        text: format!("In the named-field struct {name}, `{}` appears after the struct name and any generic parameters and before the fields in braces. It states requirements that must hold for the struct's type arguments or other types named in the clause. For `where T: Copy`, if `Copy` resolves to Rust's standard trait, the chosen T must implement Copy, allowing values of T to be copied implicitly where Rust permits; the bound alone does not copy a field or make the entire struct Copy. The written trait names and type arguments still require resolution and checking. Tuple structs place their where clause after their parenthesized field list instead.", src.norm(clause)),
        span: src.span(clause), type_info: None,
    })
}
