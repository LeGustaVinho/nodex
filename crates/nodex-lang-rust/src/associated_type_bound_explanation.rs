//! Explain bounds on associated types inside a trait's generic arguments.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Inspect the declaration header without claiming the spelled names are resolved.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if matches!(node.kind(), "source_file" | "block") { return Vec::new(); }
    let body = node.child_by_field_name("body");
    let mut pending = vec![node];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if Some(part) == body && part != node || matches!(part.kind(), "block" | "declaration_list" | "token_tree") { continue; }
        if part.kind() == "generic_type" {
            if let (Some(trait_name), Some(args)) = (part.child_by_field_name("type"), part.child_by_field_name("type_arguments")) {
                let entries = named_children(args);
                for pair in entries.windows(2) {
                    let (associated, bounds) = (pair[0], pair[1]);
                    if associated.kind() != "type_identifier" || bounds.kind() != "trait_bounds" { continue; }
                    let written = src.norm(bounds);
                    let requirements = written.trim_start_matches(':').trim();
                    if requirements.is_empty() { continue; }
                    let send = if requirements.split(|c: char| !c.is_alphanumeric() && c != '_').any(|name| name == "Send") {
                        " If Send resolves to the standard Send trait, values of that associated type can be transferred between threads; the name alone does not establish which trait it denotes."
                    } else { "" };
                    result.push(ContractItem {
                        group: "Associated type bound".into(),
                        text: format!("In {}, the colon after {} requires the associated type {} of {} to satisfy {}. This constrains the associated type rather than setting it equal to a specific type; the enclosing bound applies to the implementing type.{} Name resolution determines which trait and associated item these names denote.", src.norm(part), src.norm(associated), src.norm(associated), src.norm(trait_name), requirements, send),
                        span: src.index.span(src.file, associated.start_byte(), bounds.end_byte()),
                        type_info: None,
                    });
                }
            }
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    result
}
