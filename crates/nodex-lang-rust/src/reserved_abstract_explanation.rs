//! Explain the reserved word `abstract` even in recovered invalid syntax.
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
        // Nested modules have their own symbol; macro token trees may legally
        // contain keywords as arbitrary tokens and are interpreted by the macro.
        if matches!(node.kind(), "mod_item" | "token_tree" | "line_comment" | "block_comment" | "string_literal" | "raw_string_literal" | "char_literal") { continue; }
        if matches!(node.kind(), "identifier" | "type_identifier" | "abstract") && src.text(node) == "abstract" {
            items.push(ContractItem {
                group: "Reserved keyword abstract".into(),
                text: "`abstract` is a reserved Rust keyword; it does not declare an abstract class, method, or other construct in Rust. Using it as an ordinary identifier or as a declaration modifier is invalid Rust syntax, even if a recovering parser shows a partial structure. If a name must literally be `abstract`, write the raw identifier `r#abstract` where raw identifiers are permitted. A trait can express shared behavior, but it is not a direct equivalent of a C# abstract class. This message does not validate the rest of the code.".into(),
                span: src.span(node), type_info: None,
            });
            continue;
        }
        stack.extend(children(node));
    }
    items.reverse();
    items
}
