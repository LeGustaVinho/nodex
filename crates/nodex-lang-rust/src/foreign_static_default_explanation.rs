//! Describe foreign static declarations on their enclosing module.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn declarations(src: Src<'_>, external: Node<'_>) -> Vec<ContractItem> {
    let Some(body) = external.child_by_field_name("body") else { return Vec::new(); };
    named_children(body).into_iter().filter(|item| item.kind() == "static_item" && !item.has_error()).filter_map(|item| {
        // Some grammar versions place a `safe`/`unsafe` qualifier outside the
        // static_item. Avoid describing such explicitly qualified declarations.
        let start = item.prev_sibling().map_or(body.start_byte() + 1, |prev| prev.end_byte());
        let prefix = src.text.get(start..item.start_byte())?.trim();
        if prefix.split_whitespace().last().is_some_and(|word| matches!(word, "safe" | "unsafe")) { return None; }
        let name = item.child_by_field_name("name")?;
        let ty = item.child_by_field_name("type")?;
        let mutable = named_children(item).iter().any(|child| child.kind() == "mutable_specifier");
        Some(ContractItem {
            group: "Foreign static declaration".into(),
            text: format!("`{}` declares a foreign static named {} with Rust type {}; it refers to storage supplied by linked code rather than defining or initializing it here. A foreign static without an explicit safety qualifier is unsafe to access, so accessing it requires an unsafe context and the programmer must uphold the declaration's type and other safety requirements. {} The enclosing extern block declares an external interface; its ABI describes calls to foreign functions, not a calling convention for this variable. The source alone does not prove the symbol exists, links successfully, or has the declared type and valid value.", src.norm(item), src.text(name), src.text(ty), if mutable { "`mut` permits mutation subject to Rust's safety rules." } else { "No `mut` is written, so Rust does not permit ordinary mutation through this declaration." }),
            span: src.span(item), type_info: None,
        })
    }).collect()
}

/// The structure pass does not expose foreign statics as symbols, so attach
/// each declaration to the immediately enclosing source module instead.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    named_children(container).into_iter().filter(|item| item.kind() == "foreign_mod_item")
        .flat_map(|external| declarations(src, external)).collect()
}
