//! Explain a generic tuple struct with a where clause after its fields.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "struct_item" || node.has_error() { return None; }
    let name = node.child_by_field_name("name")?;
    let parameters = node.child_by_field_name("type_parameters")?;
    let body = node.child_by_field_name("body")?;
    if body.kind() != "ordered_field_declaration_list" { return None; }
    let clause = named_children(node).into_iter().find(|part| part.kind() == "where_clause" && part.start_byte() >= body.end_byte())?;
    let fields: Vec<_> = named_children(body).into_iter().filter(|part| !part.is_extra() && !matches!(part.kind(), "visibility_modifier" | "attribute_item")).map(|part| src.norm(part)).collect();
    let predicates: Vec<_> = named_children(clause).into_iter().filter(|part| part.kind() == "where_predicate").collect();
    if fields.is_empty() || predicates.is_empty() { return None; }
    let copy = predicates.iter().any(|predicate| predicate.child_by_field_name("bounds").is_some_and(|bounds| named_children(bounds).into_iter().any(|bound| src.norm(bound) == "Copy")));
    Some(ContractItem {
        group: "Tuple struct trailing where clause".into(),
        text: format!("`{}` declares a tuple struct with generic parameters `{}` and positional fields of types {}. Field positions start at zero; the type name can act as a constructor when it is visible. In a tuple struct, the `where` clause appears after the parenthesized fields and before the semicolon: `{}` constrains the generic arguments for the whole struct, not just the final field. {}The compiler checks these requirements whenever applicable; this declaration does not construct a value.", src.norm(name), src.norm(parameters), fields.join(", "), src.norm(clause), if copy { "If `Copy` resolves to Rust's standard `Copy` trait, the relevant type must support implicit bitwise copying; the bound does not automatically make the struct itself `Copy`. The name alone does not establish trait resolution. " } else { "The exact bounds depend on name resolution. " }),
        span: src.span(clause), type_info: None,
    })
}
