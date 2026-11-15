//! Explain semicolon-separated `$(...);*` matcher repetitions.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "macro_definition" { return Vec::new(); }
    let mut items = Vec::new();
    for rule in named_children(node).into_iter().filter(|item| item.kind() == "macro_rule") {
        let Some(matcher) = rule.child_by_field_name("left") else { continue; };
        let mut pending = vec![matcher];
        while let Some(part) = pending.pop() {
            if part.kind() == "token_repetition_pattern" && !part.has_error() {
                let tokens: Vec<_> = children(part).into_iter().filter(|token| !token.is_extra()).collect();
                // The grammar leaves the separator as a gap, not a child token.
                // Restrict the gap to a literal semicolon (plus whitespace), so
                // comments or other separators never produce a false positive.
                if tokens.len() >= 2 && tokens[tokens.len()-2].kind() == ")" && tokens[tokens.len()-1].kind() == "*"
                    && src.text.get(tokens[tokens.len()-2].end_byte()..tokens[tokens.len()-1].start_byte()).is_some_and(|gap| gap.trim() == ";") {
                    items.push(ContractItem {
                        group: "Semicolon-separated macro matcher repetition".into(),
                        text: format!("{} is a repetition in a macro matcher. $(...) groups the pattern to repeat, the semicolon separates adjacent matches, and * allows zero or more matches. No separator is required for zero or one match; this repetition by itself does not allow a trailing semicolon after the final match. Metavariables inside can capture each matched fragment for the expansion. This repeats matching of input syntax while the macro expands, not a runtime loop; the transcriber must separately describe any repeated output.", src.norm(part)),
                        span: src.span(part), type_info: None,
                    });
                }
            }
            pending.extend(named_children(part).into_iter().rev());
        }
    }
    items
}
