//! Explain `mut` on a simple named function parameter binding.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") || node.has_error() { return Vec::new(); }
    let Some(parameters) = node.child_by_field_name("parameters") else { return Vec::new(); };
    named_children(parameters).into_iter().filter_map(|parameter| {
        if parameter.kind() != "parameter" { return None; }
        let pattern = parameter.child_by_field_name("pattern")?;
        let (name, keyword) = match pattern.kind() {
            "identifier" => (pattern, children(parameter).into_iter().find(|part| part.kind() == "mutable_specifier")?),
            "mut_pattern" => {
                let name = named_children(pattern).into_iter().find(|part| part.kind() == "identifier")?;
                let keyword = children(pattern).into_iter().find(|part| part.kind() == "mutable_specifier" || part.kind() == "mut")?;
                (name, keyword)
            }
            _ => return None,
        };
        if keyword.is_missing() { return None; }
        Some(ContractItem {
            group: "Mutable parameter binding".into(),
            text: format!("`mut` makes {} a mutable local parameter binding: inside the function body it may be reassigned or modified, subject to Rust's usual rules. The argument is supplied to that binding by value; whether it is copied or moved depends on its type. This does not make the caller's own binding mutable. If the parameter contains a reference, whether the referent may be changed depends on that reference's type, not on `mut` on the parameter name. The keyword permits mutation but does not mean mutation actually occurs.", src.norm(name)),
            span: src.span(keyword), type_info: None,
        })
    }).collect()
}
