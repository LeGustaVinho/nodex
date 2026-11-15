//! Explain `options(nomem)` in an apparent inline-assembly invocation.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return Vec::new(); }
    let Some(path) = node.child_by_field_name("macro") else { return Vec::new(); };
    if src.norm(path).rsplit("::").next() != Some("asm") { return Vec::new(); }
    let Some(tree) = named_children(node).into_iter().find(|part| part.kind() == "token_tree") else { return Vec::new(); };
    let parts: Vec<_> = children(tree).into_iter().filter(|part|
        !part.is_extra() && !matches!(part.kind(), "(" | ")" | "[" | "]" | "{" | "}")).collect();
    let mut result = Vec::new();
    for group in parts.split(|part| part.kind() == ",") {
        let [options, arguments] = group else { continue; };
        if src.text(*options) != "options" || arguments.kind() != "token_tree" || !src.text(*arguments).starts_with('(') { continue; }
        let entries: Vec<_> = children(*arguments).into_iter().filter(|part|
            !part.is_extra() && !matches!(part.kind(), "(" | ")")).collect();
        for option in entries.split(|part| part.kind() == ",") {
            let [name] = option else { continue; };
            if src.text(*name) != "nomem" { continue; }
            result.push((*name, ReadingContent::Detail {
                label: "Assembly does not access outside memory".into(),
                text: "If this invocation resolves to Rust's standard `asm!` macro, `options(nomem)` promises that the assembly does not read or write memory accessible outside the assembly code, and does not synchronize with other threads through memory or fences. The compiler may optimize based on that promise. The author of the unsafe assembly must uphold it; violating it can cause undefined behavior. `nomem` alone does not say the assembly has no other effects, nor does it mean `nostack`. The spelling alone does not prove which macro is called or that the assembly obeys its obligations.".into(),
                definition: None,
                borrow: false,
            }));
        }
    }
    result
}
