//! Explain Rust's `forbid(unsafe_code)` lint attribute and compiler overrides.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn contains_unsafe_code(src: Src<'_>, item: Node<'_>) -> bool {
    if !matches!(item.kind(), "attribute_item" | "inner_attribute_item") || item.has_error() { return false; }
    let Some(attr) = named_children(item).into_iter().find(|part| part.kind() == "attribute") else { return false };
    let Some(name) = named_children(attr).into_iter().find(|part| part.kind() == "identifier") else { return false };
    if src.text(name) != "forbid" { return false; }
    let Some(args) = attr.child_by_field_name("arguments") else { return false };
    let tokens: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
    if tokens.len() < 3 || tokens[0].kind() != "(" || tokens.last().is_none_or(|part| part.kind() != ")") { return false; }
    tokens[1..tokens.len()-1].split(|part| part.kind() == ",").any(|entry| {
        entry.len() == 1 && entry[0].kind() == "identifier" && src.text(entry[0]) == "unsafe_code"
    })
}

fn item(src: Src<'_>, attr: Node<'_>, inner: bool) -> ContractItem {
    let scope = if inner { "This inner attribute applies to the enclosing module or crate and its contents." } else { "This outer attribute applies to the following item and its contents." };
    ContractItem {
        group: "Forbid unsafe-code lint".into(),
        text: format!("{scope} It sets rustc's unsafe_code lint to forbid in that scope. When the lint detects unsafe code, such as an unsafe block or unsafe declaration, forbid normally makes the diagnostic an error and prevents compilation; nested source attributes such as allow or warn cannot lower a forbidden lint. Compiler command-line controls can override the practical severity: for example, --cap-lints warn caps even forbid to a warning, and --force-warn can force a warning. This is a compile-time lint setting, not a runtime operation; its presence alone does not establish the active compiler flags or prove that dependencies contain no unsafe code."),
        span: src.span(attr), type_info: None,
    }
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if matches!(node.kind(), "attribute_item" | "inner_attribute_item") { return Vec::new(); }
    let mut result = Vec::new();
    // Outer attributes are siblings attached to the immediately following declaration.
    if node.kind() != "source_file" {
        let mut previous = node.prev_named_sibling();
        while let Some(attr) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
            previous = attr.prev_named_sibling();
            if contains_unsafe_code(src, attr) { result.push(item(src, attr, false)); }
        }
    }
    // Inner attributes belong to this module/crate, never its next declaration.
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    if let Some(container) = container {
        for attr in named_children(container).into_iter().filter(|part| part.kind() == "inner_attribute_item") {
            if contains_unsafe_code(src, attr) { result.push(item(src, attr, true)); }
        }
    }
    result
}
