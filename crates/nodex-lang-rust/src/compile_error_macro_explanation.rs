//! Explain explicit compile-time diagnostics without assuming macro resolution.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::{kit::truncate, syntax::{children, named_children, Src}};
use tree_sitter::Node;

/// Only match a qualified or unqualified spelling with one written string
/// literal. Other token forms and macro expansion require separate analysis.
fn details<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, String)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let macro_name = node.child_by_field_name("macro")?;
    let path = src.norm(macro_name);
    if path.rsplit("::").next() != Some("compile_error") { return None; }
    let tree = named_children(node).into_iter().find(|part| part.kind() == "token_tree" && !part.has_error())?;
    let parts: Vec<_> = children(tree).into_iter().filter(|part| !part.is_extra()).collect();
    if parts.len() < 3 { return None; }
    let opening = parts.first()?.kind();
    let closing = parts.last()?.kind();
    if !matches!((opening, closing), ("(", ")") | ("[", "]") | ("{", "}")) { return None; }
    let contents = &parts[1..parts.len()-1];
    let literal = match contents {
        [literal] if literal.kind() == "string_literal" => *literal,
        [literal, comma] if literal.kind() == "string_literal" && comma.kind() == "," => *literal,
        _ => return None,
    };
    Some((literal, format!("If {}! resolves to Rust's standard compile_error macro and this invocation is not removed by conditional compilation, expanding it makes the compiler report an error using the string literal {} as the diagnostic message. This intentionally prevents a successful compilation; it is not a runtime print or a branch that waits until the surrounding function is called. A #[cfg(...)] condition can exclude code before expansion. The spelling alone does not establish which macro was resolved; another macro of the same name may behave differently.", truncate(&path, 100), truncate(src.text(literal), 100))))
}

pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    let (literal, text) = details(src, node)?;
    Some((literal, ReadingContent::Detail { label: "Emit a compile-time error".into(), text, definition: None, borrow: false }))
}

/// Module-level invocations have no regular flow node. Expose direct item
/// invocations on their containing module and initializer invocations on the
/// owning const/static symbol without descending into nested declarations.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let roots = match node.kind() {
        "source_file" => named_children(node),
        "mod_item" => node.child_by_field_name("body").map(named_children).unwrap_or_default(),
        "const_item" | "static_item" => node.child_by_field_name("value").into_iter().collect(),
        _ => return Vec::new(),
    };
    let mut items = Vec::new();
    let mut pending = roots;
    while let Some(part) = pending.pop() {
        if let Some((literal, text)) = details(src, part) {
            items.push(ContractItem { group: "Compile-time error macro".into(), text, span: src.span(literal), type_info: None });
            continue;
        }
        // Traverse only an initializer's own expression, or a direct module
        // expression statement. Do not scan functions, nested modules or tokens.
        if matches!(node.kind(), "const_item" | "static_item") {
            if matches!(part.kind(), "token_tree" | "closure_expression" | "function_item" | "async_block") { continue; }
            pending.extend(named_children(part));
        } else if part.kind() == "expression_statement" {
            pending.extend(named_children(part).into_iter().filter(|child| child.kind() == "macro_invocation"));
        }
    }
    items.reverse();
    items
}
