//! Surface the reserved word `final` even in syntactically invalid Rust.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Scan a module's syntax, stopping at nested modules and opaque token trees.
/// Macro input is token data, so the compiler may legitimately accept reserved
/// words there; treating those tokens as Rust identifiers would be misleading.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    let mut stack = vec![container];
    let mut items = Vec::new();
    while let Some(part) = stack.pop() {
        if matches!(part.kind(), "mod_item" | "macro_definition" | "macro_invocation" | "token_tree" | "string_literal" | "raw_string_literal" | "char_literal" | "line_comment" | "block_comment") { continue; }
        if matches!(part.kind(), "identifier" | "type_identifier" | "field_identifier" | "shorthand_field_identifier") && src.text(part) == "final" {
            items.push(ContractItem {
                group: "Reserved Rust word".into(),
                text: "`final` is a reserved Rust keyword: it has no current language behavior and cannot be used as an ordinary identifier or a C#-style modifier. In identifier positions Rust permits the raw spelling `r#final`, which names an identifier rather than enabling a modifier. This occurrence is shown even if the surrounding Rust syntax is invalid; compilation still checks the complete program. Words inside comments, strings, and macro token input are not interpreted as ordinary Rust identifiers here.".into(),
                span: src.span(part), type_info: None,
            });
        }
        stack.extend(named_children(part).into_iter().rev());
    }
    items
}
