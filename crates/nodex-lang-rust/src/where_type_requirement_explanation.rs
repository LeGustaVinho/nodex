//! Explain written type constraints in a `where` clause.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "union_item" | "trait_item" | "type_item" | "impl_item") {
        return Vec::new();
    }
    let mut result = Vec::new();
    for clause in named_children(declaration).into_iter().filter(|n| n.kind() == "where_clause") {
        for predicate in named_children(clause).into_iter().filter(|n| n.kind() == "where_predicate") {
            let Some(target) = predicate.child_by_field_name("left").filter(|n| n.kind() != "lifetime") else { continue; };
            let Some(bounds) = predicate.child_by_field_name("bounds") else { continue; };
            let requirements = named_children(bounds).into_iter().filter(|n| n.kind() != "lifetime").collect::<Vec<_>>();
            if requirements.is_empty() { continue; }
            let target_text = src.norm(target);
            let written = src.norm(bounds);
            let requirements_text = written.trim_start_matches(':').trim();
            let clone_note = if requirements.iter().any(|bound| src.norm(*bound) == "Clone") {
                " If Clone resolves to Rust's standard Clone trait, it supplies an explicit way to create a duplicate value; the spelling alone does not prove which item the name denotes."
            } else { "" };
            result.push(ContractItem {
                group: "Where-clause type requirement".into(),
                text: format!("The where clause lists requirements for this declaration. Here {target_text}: {requirements_text} means the type {target_text} must satisfy every listed bound whenever this declaration is used or implemented; + joins multiple requirements. The compiler checks these constraints; writing the clause does not call a method or duplicate a value.{clone_note}"),
                span: src.span(predicate), type_info: None,
            });
        }
    }
    result
}
