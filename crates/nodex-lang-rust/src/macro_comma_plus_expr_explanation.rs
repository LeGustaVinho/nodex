//! Explain comma-separated one-or-more expression repetitions in macro matchers.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if declaration.kind() != "macro_definition" { return Vec::new(); }
    let mut result = Vec::new();
    for rule in named_children(declaration).into_iter().filter(|part| part.kind() == "macro_rule" && !part.has_error()) {
        // Inspect matcher syntax only: a repetition in the transcriber means something else.
        let Some(matcher) = rule.child_by_field_name("left") else { continue };
        let mut pending = vec![matcher];
        while let Some(node) = pending.pop() {
            if node.kind() == "token_repetition_pattern" && !node.has_error() {
                let tokens = children(node);
                let close = tokens.iter().find(|part| part.kind() == ")").copied();
                let plus = tokens.iter().rev().find(|part| part.kind() == "+").copied();
                let parts: Vec<_> = named_children(node).into_iter().filter(|part| !part.is_extra()).collect();
                if let (Some(close), Some(plus), [capture]) = (close, plus, parts.as_slice()) {
                    let separator = src.text.get(close.end_byte()..plus.start_byte()).unwrap_or("").trim();
                    if separator == "," && capture.kind() == "token_binding_pattern" {
                        if let (Some(name), Some(fragment)) = (capture.child_by_field_name("name"), capture.child_by_field_name("type")) {
                            if src.text(fragment) == "expr" {
                                let name = src.norm(name);
                                result.push(ContractItem {
                                    group: "One-or-more expression repetition".into(),
                                    text: format!("In a macro_rules! matcher, $(...) repeats the enclosed pattern. {name}:expr matches an expression as syntax and records each match under the metavariable {name}. The comma after the closing parenthesis separates successive matches, and + requires one or more matches: an empty list does not satisfy this repetition. The repetition itself does not consume a trailing comma, although the surrounding matcher can allow one separately. A matching transcriber can reuse the captured sequence according to macro repetition rules. Matching syntax does not execute the expressions; evaluation happens only as determined by the expanded code. The exact expr grammar depends on the macro definition's Rust edition."),
                                    span: src.span(node), type_info: None,
                                });
                            }
                        }
                    }
                }
            }
            pending.extend(named_children(node).into_iter().rev());
        }
    }
    result
}
