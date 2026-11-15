//! Explain an unescaped `do` token even in a malformed function body.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let mut pending = vec![node];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if part.is_extra() || part.is_missing() || matches!(part.kind(), "token_tree" | "string_literal" | "raw_string_literal" | "char_literal" | "line_comment" | "block_comment") { continue; }
        if part != node && matches!(part.kind(), "function_item" | "function_signature_item" | "macro_definition") { continue; }
        if matches!(part.kind(), "identifier" | "type_identifier" | "do") && src.text(part) == "do" {
            result.push(ContractItem {
                group: "Reserved keyword do".into(),
                text: "`do` is a reserved Rust keyword; Rust does not provide a `do { ... } while ...` statement. This unescaped use is not a normal Rust identifier or a working do-while loop. For repeated execution, use `loop` or `while` and express the desired condition explicitly. If a name is intended, `r#do` is a raw identifier and its name is `do`. The surrounding source may be incomplete or contain other syntax errors; this explanation does not make it valid Rust.".into(),
                span: src.span(part), type_info: None,
            });
        }
        pending.extend(children(part));
    }
    result.reverse();
    result
}
