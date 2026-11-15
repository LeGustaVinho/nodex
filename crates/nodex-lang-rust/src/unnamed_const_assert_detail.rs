//! Explain an assertion inside an unnamed unit constant.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "const_item" || node.has_error() { return Vec::new(); }
    let Some(name) = node.child_by_field_name("name") else { return Vec::new(); };
    if src.text(name) != "_" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    if ty.kind() != "unit_type" { return Vec::new(); }
    let Some(block) = node.child_by_field_name("value") else { return Vec::new(); };
    if block.kind() != "block" { return Vec::new(); }
    named_children(block).into_iter().filter_map(|statement| {
        if statement.kind() != "expression_statement" { return None; }
        let invocation = named_children(statement).into_iter().find(|part| part.kind() == "macro_invocation")?;
        let path = invocation.child_by_field_name("macro")?;
        let name: String = src.text(path).chars().filter(|ch| !ch.is_whitespace()).collect();
        if !matches!(name.as_str(), "assert" | "std::assert" | "::std::assert" | "core::assert" | "::core::assert") { return None; }
        Some(ContractItem {
            group: "Unnamed constant assertion".into(),
            text: "`const _: () = { ... };` declares an unnamed constant with unit type, evaluated in a compile-time constant context; `_` gives it no usable name and `()` is the value type. If this invocation resolves to Rust's standard `assert!`, it checks its condition during constant evaluation: a false condition causes a panic and compilation fails for that evaluated constant. The assertion is not a runtime check. Macro names can resolve differently, so the spelling alone does not prove that standard assertion behavior applies; configuration and constant-evaluation rules also affect whether the declaration is compiled successfully.".into(),
            span: src.span(path), type_info: None,
        })
    }).collect()
}
