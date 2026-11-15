//! Explain the `literal` fragment specifier in a macro_rules matcher.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Inspect only each rule's matcher. The transcriber is a token tree and must
/// not be mistaken for a binding declaration, even if it contains `:literal`.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "macro_definition" || node.has_error() { return Vec::new(); }
    let mut output = Vec::new();
    for rule in named_children(node).into_iter().filter(|part| part.kind() == "macro_rule") {
        let Some(matcher) = rule.child_by_field_name("left") else { continue; };
        let mut pending = vec![matcher];
        while let Some(part) = pending.pop() {
            if part.kind() == "token_binding_pattern" {
                if let (Some(name), Some(fragment)) = (part.child_by_field_name("name"), part.child_by_field_name("type")) {
                    if src.text(fragment) == "literal" {
                        let repeated = std::iter::successors(part.parent(), |parent| parent.parent())
                            .take_while(|parent| *parent != matcher)
                            .any(|parent| parent.kind() == "token_repetition_pattern");
                        let repetition = if repeated { " This capture appears inside a repetition, so each matched iteration can capture its own literal; the transcriber must use repetitions consistently." } else { "" };
                        output.push(ContractItem {
                            group: "Literal macro fragment".into(),
                            text: format!("In this macro matcher, {}:literal declares a macro metavariable {}. The literal fragment accepts a literal expression, optionally preceded by -, such as a number, character, string, or boolean literal; it does not match an arbitrary calculation like 1 + 2 as one literal fragment. Matching consumes input tokens during macro expansion, not a runtime function argument or a variable evaluation. The rule's transcriber can insert the captured fragment using {}; {} is a macro binding, not an ordinary Rust variable. Literal tokens can carry suffixes accepted by the macro input even if the resulting tokens would later be invalid as a Rust literal expression.{}", src.norm(name), src.norm(name), src.norm(name), src.norm(name), repetition),
                            span: src.span(fragment), type_info: None,
                        });
                    }
                }
                // A binding is one fragment; its children cannot contain nested matchers.
                continue;
            }
            pending.extend(named_children(part).into_iter().rev());
        }
    }
    output
}
