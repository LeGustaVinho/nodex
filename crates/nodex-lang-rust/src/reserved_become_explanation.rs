//! Explain Rust's reserved `become` token and experimental tail calls.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, module: Node<'_>) -> Vec<ContractItem> {
    let container = match module.kind() {
        "source_file" => Some(module),
        "mod_item" => module.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    let mut stack = children(container);
    let mut items = Vec::new();
    while let Some(node) = stack.pop() {
        if matches!(node.kind(), "mod_item" | "token_tree" | "line_comment" | "block_comment" | "string_literal" | "raw_string_literal" | "char_literal") { continue; }
        if matches!(node.kind(), "identifier" | "type_identifier" | "become") && src.text(node) == "become" {
            items.push(ContractItem {
                group: "Reserved keyword become".into(),
                text: "`become` is a reserved Rust keyword, so it is not an ordinary identifier; `r#become` can be used as a raw identifier where permitted. The experimental, incomplete `explicit_tail_calls` feature uses `become function(args)` for an explicit tail call: execution transfers to the called function as the current function ends, generally replacing the caller's stack frame instead of accumulating another one. This feature requires an appropriate nightly compiler and feature gate, has restrictions on the caller and callee, and its behavior can change; a callee with `#[track_caller]` is a documented exception to the tail-call guarantee. It also changes local drop order relative to an ordinary return call. Seeing the token alone does not establish that this is a valid tail call, that the feature is enabled, or that the rest of the code compiles.".into(),
                span: src.span(node), type_info: None,
            });
            continue;
        }
        stack.extend(children(node));
    }
    items.reverse();
    items
}
