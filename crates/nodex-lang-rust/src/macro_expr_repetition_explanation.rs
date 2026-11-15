//! Explain comma-separated expression repetition in `macro_rules!` matchers.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "token_repetition_pattern" || node.has_error() { return None; }
    let named: Vec<_> = named_children(node).into_iter().filter(|part| !part.is_extra()).collect();
    if named.len() != 1 || named[0].kind() != "token_binding_pattern" { return None; }
    let binding = named[0];
    let name = binding.child_by_field_name("name")?;
    let fragment = binding.child_by_field_name("type")?;
    if src.text(fragment) != "expr" { return None; }
    let parts = children(node);
    let close = parts.iter().rfind(|part| part.kind() == ")" && !part.is_missing())?;
    let star = parts.iter().rfind(|part| part.kind() == "*" && !part.is_missing())?;
    if close.end_byte() > star.start_byte() || src.text[close.end_byte()..star.start_byte()].trim() != "," { return None; }
    Some(ContractItem {
        group: "Comma-separated expression repetition".into(),
        text: format!("In the macro matcher {}, $() repeats the pattern inside it. {}:expr captures one expression on each iteration; the comma separates successive expressions and * allows zero or more iterations. For example, the repeated part can match no expressions, one expression, or several expressions separated by commas. This repetition alone does not consume a trailing comma; additional matcher syntax such as $(,)? may allow one. Matching processes tokens during macro expansion and does not itself evaluate the expressions. The transcriber can use the captures in a compatible repetition; its expansion determines whether and how often generated expressions run. The syntax accepted by expr also depends on the edition of the macro definition.", src.norm(node), src.norm(name)),
        span: src.span(*star), type_info: None,
    })
}

/// Inspect matcher trees only, including nested matcher repetitions. Do not
/// scan the right-hand transcriber, where `$()` describes expansion instead.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "macro_definition" || node.has_error() { return Vec::new(); }
    let mut output = Vec::new();
    for rule in named_children(node).into_iter().filter(|part| part.kind() == "macro_rule") {
        let Some(matcher) = rule.child_by_field_name("left") else { continue; };
        let mut pending = vec![matcher];
        while let Some(part) = pending.pop() {
            if let Some(item) = describe(src, part) { output.push(item); }
            pending.extend(named_children(part).into_iter().rev());
        }
    }
    output
}
