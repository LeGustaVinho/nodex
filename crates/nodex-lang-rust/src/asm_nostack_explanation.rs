//! Explain `options(nostack)` in an `asm!` invocation without resolving the macro.
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
    // Split only top-level commas: a template string or nested group stays opaque.
    for part in &parts[1..parts.len()-1] {
        if part.kind() == "," { segments.push(Vec::new()); } else { segments.last_mut().unwrap().push(*part); }
    }
    let mut output = Vec::new();
    // `asm!` requires at least one template before its operands/options.
    for segment in segments.into_iter().skip(1) {
        let [options, group] = segment.as_slice() else { continue; };
        if src.text(*options) != "options" || group.kind() != "token_tree" || group.has_error() { continue; }
        let group_parts: Vec<_> = children(*group).into_iter().filter(|part| !part.is_extra()).collect();
        if group_parts.len() < 3 || group_parts.first().is_none_or(|part| part.kind() != "(") || group_parts.last().is_none_or(|part| part.kind() != ")") { continue; }
        let contents = &group_parts[1..group_parts.len()-1];
        if contents.iter().enumerate().any(|(i, part)| if i % 2 == 0 { part.kind() != "identifier" } else { part.kind() != "," }) { continue; }
        for site in contents.iter().copied().filter(|part| part.kind() == "identifier" && src.text(*part) == "nostack") {
            output.push((site, ReadingContent::Detail {
                label: "Assembly nostack promise".into(),
                text: "If this invocation resolves to Rust's standard `asm!` and is accepted, `options(nostack)` promises that the assembly does not push data to the stack or write to the stack red zone when the target has one. Without this option, the compiler guarantees stack alignment suitable for a function call on entry; with it, that particular guarantee is not provided. This is an unsafe obligation on the assembly author, not a check that the instructions satisfy it: violating the promise can cause undefined behavior. It does not mean the assembly cannot access any memory, and the written macro name alone does not prove resolution.".into(),
                definition: None,
                borrow: false,
            }));
        }
    }
    output
}
