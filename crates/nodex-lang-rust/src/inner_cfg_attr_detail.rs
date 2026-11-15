//! Explain a crate-root conditional lint allowance.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "source_file" { return Vec::new(); }
    named_children(node).into_iter().filter_map(|part| {
        if part.kind() != "inner_attribute_item" || part.has_error() { return None; }
        let meta = named_children(part).into_iter().find(|child| child.kind() == "attribute")?;
        let name = named_children(meta).into_iter().next()?;
        if src.text(name) != "cfg_attr" { return None; }
        let args = meta.child_by_field_name("arguments")?;
        let parts: Vec<_> = children(args).into_iter().filter(|child| !child.is_extra()).collect();
        if parts.len() < 2 { return None; }
        let segments: Vec<_> = parts[1..parts.len()-1].split(|child| child.kind() == ",").filter(|segment| !segment.is_empty()).collect();
        let [predicate, attribute] = segments.as_slice() else { return None; };
        let [feature, equals, value] = *predicate else { return None; };
        if src.text(*feature) != "feature" || equals.kind() != "=" || value.kind() != "string_literal" { return None; }
        let [allow, group] = *attribute else { return None; };
        if src.text(*allow) != "allow" || group.kind() != "token_tree" { return None; }
        let inner: Vec<_> = children(*group).into_iter().filter(|child| !child.is_extra()).collect();
        if !matches!(inner.as_slice(), [open, lint, close] if open.kind() == "(" && src.text(*lint) == "dead_code" && close.kind() == ")") { return None; }
        Some(ContractItem {
            group: "Conditional crate dead-code allowance".into(),
            text: format!("This inner attribute applies to the crate root. At compile time `cfg_attr` checks whether the configuration has feature {}; if true, it applies `allow(dead_code)` to the crate and its scope, normally suppressing that lint's warnings about unused, unexported code. If false, this attribute adds no such allowance. It does not remove code or change runtime behavior. The source alone does not tell us whether the feature is enabled. Lint levels and command-line options can affect the final result; an enclosing `forbid` or command-line controls can prevent or override the allowance.", src.text(*value)),
            span: src.span(part), type_info: None,
        })
    }).collect()
}
