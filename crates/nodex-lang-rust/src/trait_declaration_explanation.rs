//! Beginner-facing description of a trait declaration.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "trait_item" { return None; }
    let name = node.child_by_field_name("name")?;
    let body = node.child_by_field_name("body")?;
    let mut descriptions = Vec::new();
    for method in named_children(body).into_iter().filter(|item|
        matches!(item.kind(), "function_item" | "function_signature_item")) {
        let Some(method_name) = method.child_by_field_name("name") else { continue };
        let requirement = if method.kind() == "function_signature_item" {
            "has no body here, so each implementation must provide it"
        } else {
            "has a default body here, which an implementation may override"
        };
        let receiver = method.child_by_field_name("parameters")
            .into_iter().flat_map(named_children)
            .find(|parameter| parameter.kind() == "self_parameter")
            .map(|parameter| src.norm(parameter));
        let receiver_explanation = match receiver.as_deref() {
            Some("&self") => " Its &self parameter borrows the value being used for shared access; it does not take ownership.",
            _ => "",
        };
        descriptions.push(format!("The method {} {requirement}.{receiver_explanation}", src.norm(method_name)));
    }
    let methods = if descriptions.is_empty() { String::new() } else { format!(" {}", descriptions.join(" ")) };
    Some(ContractItem {
        group: "Trait".into(),
        text: format!("trait declares {} as a set of behavior that types can implement. It describes what an implementing type provides; declaring the trait does not create a value.{}", src.norm(name), methods),
        span: src.span(node),
        type_info: None,
    })
}
