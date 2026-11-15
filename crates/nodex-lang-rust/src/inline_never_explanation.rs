//! Explain a direct `#[inline(never)]` code-generation hint.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    // Required trait declarations have no body to inline; rustc lints the
    // attribute in that position rather than applying this codegen hint.
    if node.kind() != "function_item" || node.child_by_field_name("body").is_none() { return Vec::new(); }
    let mut previous = node.prev_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" if !part.has_error() => {
                if let Some(meta) = named_children(part).into_iter().find(|child| child.kind() == "attribute")
                    && let Some(path) = named_children(meta).into_iter().next()
                    && src.text(path) == "inline"
                    && let Some(args) = meta.child_by_field_name("arguments")
                {
                    let values: Vec<_> = children(args).into_iter().filter(|child| !child.is_extra() && !matches!(child.kind(), "(" | ")")).collect();
                    if values.len() == 1 && src.text(values[0]) == "never" {
                        result.push(ContractItem {
                            group: "Inline never hint".into(),
                            text: "`#[inline(never)]` suggests that the compiler should not inline this function: inlining means replacing a call with a copy of the callee's body in the caller. This is an optimization hint, and the compiler may ignore it; it does not stop the function from being called or change its Rust-level behavior, and it does not guarantee a particular speed or binary layout. On an async function the hint applies to the generated `poll` function; on a trait method it applies to the default body. The attribute is ignored for a function externally exported with `no_mangle` or `export_name`.".into(),
                            span: src.span(part), type_info: None,
                        });
                    }
                }
            }
            "line_comment" if !src.text(part).starts_with("//!") => {},
            "block_comment" if !src.text(part).starts_with("/*!") => {},
            _ => break,
        }
        previous = part.prev_sibling();
    }
    result.reverse();
    result
}
