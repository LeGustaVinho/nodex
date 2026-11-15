//! Explain a simple declarative macro rule that captures and emits one expression.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if declaration.kind() != "macro_definition" { return Vec::new(); }
    let mut result = Vec::new();
    for rule in named_children(declaration).into_iter().filter(|node| node.kind() == "macro_rule" && !node.has_error()) {
        let (Some(matcher), Some(template)) = (rule.child_by_field_name("left"), rule.child_by_field_name("right")) else { continue };
        if !children(matcher).iter().any(|part| part.kind() == "(") || !children(template).iter().any(|part| part.kind() == "{") { continue; }
        let left: Vec<_> = named_children(matcher).into_iter().filter(|part| !part.is_extra()).collect();
        let right: Vec<_> = named_children(template).into_iter().filter(|part| !part.is_extra()).collect();
        let ([capture], [emitted]) = (left.as_slice(), right.as_slice()) else { continue };
        if capture.kind() != "token_binding_pattern" || emitted.kind() != "metavariable" { continue; }
        let (Some(name), Some(fragment)) = (capture.child_by_field_name("name"), capture.child_by_field_name("type")) else { continue };
        if src.text(fragment) != "expr" || src.text(name) != src.text(*emitted) { continue; }
        let capture_name = src.norm(name);
        result.push(ContractItem {
            group: "Macro expression capture rule".into(),
            text: format!("This macro rule has a matcher on the left of => and an expansion template on the right. In the matcher, {capture_name}:expr captures one piece of input that parses as an expression and names that captured syntax {capture_name}; it is a macro metavariable, not a normal Rust variable. When the rule matches a macro invocation, the compiler substitutes that captured expression syntax for {capture_name} in the template during macro expansion. The braces delimit the template; they do not by themselves mean an extra Rust block is emitted. Matching the rule does not execute the expression: the expanded code determines when it runs. Which expressions match expr can depend on the macro's edition."),
            span: src.span(rule), type_info: None,
        });
    }
    result
}
