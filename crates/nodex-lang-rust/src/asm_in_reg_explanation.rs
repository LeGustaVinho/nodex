//! Explain `in(reg) expression` operands written directly in an `asm!` call.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return Vec::new(); }
    let Some(name) = node.child_by_field_name("macro") else { return Vec::new(); };
    let written_name = src.norm(name).replace(char::is_whitespace, "");
    if written_name.split("::").last() != Some("asm") { return Vec::new(); }
    let Some(tree) = named_children(node).into_iter().find(|part| part.kind() == "token_tree") else { return Vec::new(); };
    if tree.has_error() { return Vec::new(); }
    let parts: Vec<_> = children(tree).into_iter().filter(|part| !part.is_extra()).collect();
    if parts.len() < 2 { return Vec::new(); }
    let mut segments: Vec<Vec<Node<'t>>> = vec![Vec::new()];
    // Only split top-level commas. Nested groups and string literals stay opaque.
    for part in &parts[1..parts.len()-1] {
        if part.kind() == "," { segments.push(Vec::new()); } else { segments.last_mut().unwrap().push(*part); }
    }
    let mut result = Vec::new();
    for segment in segments.into_iter().skip(1) {
        let start = if segment.len() >= 2 && segment[0].kind() == "identifier" && segment[1].kind() == "=" { 2 } else { 0 };
        if segment.len() <= start + 2 || src.text(segment[start]) != "in" { continue; }
        let group = segment[start + 1];
        if group.kind() != "token_tree" || group.has_error() { continue; }
        let group_parts: Vec<_> = children(group).into_iter().filter(|part| !part.is_extra()).collect();
        if !matches!(group_parts.as_slice(), [left, middle, right] if left.kind() == "(" && src.text(*middle) == "reg" && right.kind() == ")") { continue; }
        let Some(expression) = src.text.get(segment[start + 2].start_byte()..segment.last().unwrap().end_byte()) else { continue; };
        let keyword = segment[start];
        result.push((keyword, ReadingContent::Detail {
            label: "Assembly input register".into(),
            text: format!("If this invocation resolves to Rust's standard `asm!` and is accepted for the target, `in(reg) {}` supplies an input to the assembly. Rust evaluates the expression and places its value in a register selected by the compiler from the target's `reg` class; the allocated register can be substituted into the assembly template. `in` is input-only, not an output assignment back to the expression. The exact register and supported types depend on the target and allocation. The written macro name alone does not prove resolution or that the assembly is safe.", expression.trim()),
            definition: None,
            borrow: false,
        }));
    }
    result
}
