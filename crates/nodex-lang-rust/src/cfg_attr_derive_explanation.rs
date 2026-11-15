//! Explain a direct `derive(Debug)` inside an outer `cfg_attr` attribute.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::{kit::truncate, syntax::{children, named_children, Src}};
use tree_sitter::Node;

fn is_debug_derive(src: Src<'_>, segment: &[Node<'_>]) -> bool {
    let [name, args] = segment else { return false; };
    if src.norm(*name) != "derive" || args.kind() != "token_tree" { return false; }
    let text = src.text(*args);
    let Some(body) = text.strip_prefix('(').and_then(|text| text.strip_suffix(')')) else { return false; };
    body.split(',').any(|name| name.trim() == "Debug")
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if matches!(node.kind(), "source_file" | "attribute_item" | "inner_attribute_item") { return Vec::new(); }
    let mut previous = node.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        if part.kind() == "attribute_item" {
            if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute")
                && named_children(attribute).into_iter().next().is_some_and(|path| src.norm(path) == "cfg_attr")
                && let Some(args) = attribute.child_by_field_name("arguments")
            {
                let tokens: Vec<_> = children(args).into_iter().filter(|child| !child.is_extra() && !matches!(child.kind(), "(" | ")")).collect();
                if let Some(comma) = tokens.iter().position(|child| child.kind() == ",")
                    && comma > 0
                    && tokens[comma + 1..].split(|child| child.kind() == ",").any(|segment| is_debug_derive(src, segment))
                {
                    let predicate = &src.text[tokens[0].start_byte()..tokens[comma].start_byte()];
                    result.push(ContractItem {
                        group: "Conditional derive attribute".into(),
                        text: format!("`cfg_attr` checks the configuration predicate {} for this compilation. If it is true, the listed `derive(Debug)` attribute is applied to this item; if false, that attribute is not applied. Applying derive asks the resolved derive macro to generate additional implementations at compile time. If Debug resolves to Rust's standard Debug derive, it generates debug formatting support (subject to any requirements on fields and types). The syntax alone does not establish whether the predicate is true or which macro Debug resolves to, and it does not print anything by itself.", truncate(predicate.trim(), 120)),
                        span: src.span(part), type_info: None,
                    });
                }
            }
        } else if !part.is_extra() { break; }
        previous = part.prev_named_sibling();
    }
    result.reverse();
    result
}
