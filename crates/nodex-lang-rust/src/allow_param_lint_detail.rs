//! Explain an unused-variable lint allowance attached to a parameter.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn lint<'t>(src: Src<'_>, attribute: Node<'t>) -> Option<Node<'t>> {
    if attribute.kind() != "attribute_item" || attribute.has_error() { return None; }
    let attr = named_children(attribute).into_iter().find(|n| n.kind() == "attribute")?;
    let name = named_children(attr).into_iter().find(|n| n.kind() == "identifier")?;
    if src.text(name) != "allow" { return None; }
    let args = attr.child_by_field_name("arguments")?;
    let parts: Vec<_> = children(args).into_iter().filter(|n| !n.is_extra() && !matches!(n.kind(), "(" | ")")).collect();
    parts.split(|n| n.kind() == ",").find_map(|segment| match segment {
        [candidate] if candidate.kind() == "identifier" && src.text(*candidate) == "unused_variables" => Some(*candidate),
        _ => None,
    })
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let Some(parameters) = node.child_by_field_name("parameters") else { return Vec::new(); };
    let mut attributes = Vec::new();
    let mut result = Vec::new();
    for part in named_children(parameters) {
        if part.kind() == "attribute_item" { attributes.push(part); continue; }
        if part.kind() == "parameter" {
            if let Some(pattern) = part.child_by_field_name("pattern") {
                for attribute in attributes.drain(..) {
                    if let Some(site) = lint(src, attribute) {
                        result.push(ContractItem {
                            group: "Allow unused parameter".into(),
                            text: format!("`#[allow(unused_variables)]` on parameter `{}` requests the allow level for the unused-variable lint at this parameter. It suppresses that lint's diagnostic there when the effective lint configuration permits; a surrounding forbid level or compiler lint settings can affect the result. This does not prove the parameter is unused, change its type or whether callers pass an argument, or alter runtime behavior. It controls a compile-time diagnostic, not the variable's value.", src.norm(pattern)),
                            span: src.span(site), type_info: None,
                        });
                    }
                }
            } else { attributes.clear(); }
        } else if !part.is_extra() { attributes.clear(); }
    }
    result
}
