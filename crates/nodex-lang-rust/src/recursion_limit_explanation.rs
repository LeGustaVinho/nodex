//! Explain the crate-level recursion_limit inner attribute without assuming the build setup.
//! Reference: https://doc.rust-lang.org/reference/attributes/limits.html#the-recursion_limit-attribute
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "source_file" { return Vec::new(); }
    let mut items = Vec::new();
    for attribute in named_children(node).into_iter().filter(|part| part.kind() == "inner_attribute_item" && !part.has_error()) {
        let Some(meta) = named_children(attribute).into_iter().find(|part| part.kind() == "attribute") else { continue; };
        let Some(name) = named_children(meta).into_iter().find(|part| part.kind() == "identifier") else { continue; };
        if src.text(name) != "recursion_limit" { continue; }
        let Some(value) = meta.child_by_field_name("value").filter(|value| matches!(value.kind(), "string_literal" | "raw_string_literal")) else { continue; };
        items.push(ContractItem {
            group: "Compile-time recursion limit".into(),
            text: format!("{} is an inner attribute: #![...] applies to its enclosing scope rather than just the next item. When used at the crate root with a compiler-accepted value, recursion_limit sets a maximum depth for potentially recursive compile-time operations such as macro expansion and automatic dereferencing. The requested limit is written as the string {}; the compiler must validate and interpret that value. This limit is about compilation, not how deeply the finished program may recurse at runtime. Increasing it does not prove that an expansion terminates and can require more compiler time or memory. The source alone does not prove which build is active or that this file is the crate root.", src.norm(attribute), src.text(value)),
            span: src.span(attribute),
            type_info: None,
        });
    }
    items
}
