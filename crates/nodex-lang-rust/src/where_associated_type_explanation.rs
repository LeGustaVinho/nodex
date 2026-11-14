//! Explain a bound on a projected associated type in a where clause.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let mut items = Vec::new();
    for clause in named_children(node).into_iter().filter(|part| part.kind() == "where_clause") {
        for predicate in named_children(clause) {
            let Some(left) = predicate.child_by_field_name("left") else { continue };
            if left.kind() != "scoped_type_identifier" { continue; }
            let Some(path) = left.child_by_field_name("path") else { continue };
            let Some(name) = left.child_by_field_name("name") else { continue };
            let Some(bounds) = predicate.child_by_field_name("bounds") else { continue };
            let written = src.norm(bounds);
            let requirements = written.trim_start_matches(':').trim();
            if requirements.is_empty() { continue; }
            let clone_note = if requirements.split(|ch: char| !ch.is_alphanumeric() && ch != '_').any(|name| name == "Clone") {
                " If Clone denotes Rust's standard Clone trait, satisfying it permits explicit cloning; the name alone does not prove which trait it resolves to."
            } else { "" };
            items.push(ContractItem {
                group: "Associated type requirement".into(),
                text: format!("The where clause adds a requirement to this declaration. {} selects the associated type {} through {}; the colon says that selected type must satisfy {}. This constrains the associated type, rather than requiring {} itself to satisfy those bounds.{} The compiler checks the requirement when this declaration is used.", src.norm(left), src.norm(name), src.norm(path), requirements, src.norm(path), clone_note),
                span: src.span(predicate),
                type_info: None,
            });
        }
    }
    items
}
