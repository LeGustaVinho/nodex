//! Explain a module's inner dead-code lint allowance.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "mod_item" { return Vec::new(); }
    let Some(name) = node.child_by_field_name("name") else { return Vec::new(); };
    let Some(body) = node.child_by_field_name("body") else { return Vec::new(); };
    named_children(body).into_iter().filter_map(|part| {
        if part.kind() != "inner_attribute_item" || part.has_error() { return None; }
        let attribute = named_children(part).into_iter().find(|child| child.kind() == "attribute")?;
        let path = named_children(attribute).into_iter().next()?;
        if src.text(path) != "allow" { return None; }
        let args = attribute.child_by_field_name("arguments")?;
        let tokens: Vec<_> = children(args).into_iter().filter(|child| !child.is_extra()).collect();
        let has_lint = tokens.windows(3).any(|parts| matches!(parts[0].kind(), "(" | ",") && src.text(parts[1]) == "dead_code" && matches!(parts[2].kind(), ")" | ","));
        if !has_lint { return None; }
        Some(ContractItem {
            group: "Module dead-code lint allowance".into(),
            text: format!("The inner attribute `#![allow(dead_code)]` applies to the enclosing module {} and its nested scope, normally suppressing `dead_code` lint warnings about unused, unexported code there. It does not remove code, execute it, or change runtime behavior. Nested lint settings and compiler command-line options can affect the final lint level; a higher `forbid` cannot normally be lowered by this allowance. The leading `#!` applies to the enclosing module rather than only the next function.", src.norm(name)),
            span: src.span(part), type_info: None,
        })
    }).collect()
}
