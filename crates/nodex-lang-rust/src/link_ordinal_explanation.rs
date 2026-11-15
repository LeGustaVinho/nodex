//! Explain a foreign function's Windows ordinal import attribute.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_signature_item"
        || !node.parent().and_then(|body| body.parent()).is_some_and(|parent| parent.kind() == "foreign_mod_item")
    { return Vec::new(); }
    let name = node.child_by_field_name("name").map(|name| src.norm(name)).unwrap_or_else(|| "this function".into());
    let mut previous = node.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        if part.kind() == "attribute_item" {
            if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute")
                && named_children(attribute).into_iter().next().is_some_and(|path| src.norm(path) == "link_ordinal")
                && let Some(args) = attribute.child_by_field_name("arguments")
            {
                let tokens: Vec<_> = children(args).into_iter().filter(|child| !child.is_extra() && !matches!(child.kind(), "(" | ")")).collect();
                if let [ordinal] = tokens.as_slice()
                    && ordinal.kind() == "integer_literal"
                {
                    result.push(ContractItem {
                        group: "Foreign import by ordinal".into(),
                        text: format!("`#[link_ordinal({})]` tells Rust to identify the foreign symbol for {name} by the written numeric ordinal when generating its import library, rather than by an exported symbol name. On Windows, an ordinal identifies an export in a dynamic library; it is not a function argument or return value. Rust permits this attribute only with `raw-dylib` linking, which is supported for Windows targets, and not together with `#[link_name]`. The library must provide the intended stable ordinal and a compatible ABI and signature; automatically assigned ordinals can change between library builds. The declaration alone does not prove linking or a call will succeed.", src.norm(*ordinal)),
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
