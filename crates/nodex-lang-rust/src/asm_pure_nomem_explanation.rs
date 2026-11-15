//! Explain the combination of pure and nomem assembly options.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return Vec::new(); }
    let Some(path) = node.child_by_field_name("macro") else { return Vec::new(); };
    let spelling: String = src.text(path).chars().filter(|ch| !ch.is_whitespace()).collect();
    if !matches!(spelling.as_str(), "asm" | "core::arch::asm" | "::core::arch::asm" | "std::arch::asm" | "::std::arch::asm") { return Vec::new(); }
    let Some(tree) = named_children(node).into_iter().find(|part| part.kind() == "token_tree") else { return Vec::new(); };
    let parts: Vec<_> = children(tree).into_iter().filter(|part| !part.is_extra()).collect();
    if parts.len() < 2 { return Vec::new(); }
    let mut result = Vec::new();
    for segment in parts[1..parts.len()-1].split(|part| part.kind() == ",") {
        let [keyword, group] = segment else { continue; };
        if src.text(*keyword) != "options" || group.kind() != "token_tree" || !src.text(*group).starts_with('(') { continue; }
        let entries: Vec<_> = children(*group).into_iter().filter(|part| !part.is_extra() && !matches!(part.kind(), "(" | ")")).collect();
        let values: Vec<_> = entries.split(|part| part.kind() == ",").filter_map(|entry| match entry { [item] => Some(src.text(*item)), _ => None }).collect();
        if !values.contains(&"pure") || !values.contains(&"nomem") { continue; }
        result.push((*keyword, ReadingContent::Detail {
            label: "Pure assembly without outside memory".into(),
            text: "If this invocation resolves to Rust's standard `asm!`, `options(pure, nomem)` promises that the assembly has no side effects, eventually returns, and its outputs depend only on direct input values. It must not read or write memory accessible outside the assembly or synchronize with other threads. The compiler may move it, execute it fewer times, or remove it when outputs are unused; it requires at least one non-discarded output. The author must uphold these unsafe assembly promises: violating them can cause undefined behavior. `nomem` does not prohibit using private stack memory. The written name does not prove macro resolution or that the promises hold.".into(),
            definition: None, borrow: false,
        }));
    }
    result
}
