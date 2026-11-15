//! Explain the `meta` fragment in a declarative macro matcher.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if declaration.kind() != "macro_definition" { return Vec::new(); }
    let mut result = Vec::new();
    for rule in named_children(declaration).into_iter().filter(|node| node.kind() == "macro_rule" && !node.has_error()) {
        // Traverse the matcher only. `$x:meta` written in a transcriber is not
        // a matcher declaration, even when its tokens look similar.
        let Some(matcher) = rule.child_by_field_name("left") else { continue };
        let mut pending = vec![(matcher, false)];
        while let Some((node, repeated)) = pending.pop() {
            let repeated = repeated || node.kind() == "token_repetition_pattern";
            if node.kind() == "token_binding_pattern" {
                let (Some(name), Some(fragment)) = (node.child_by_field_name("name"), node.child_by_field_name("type")) else { continue };
                if src.text(fragment) == "meta" {
                    let name = src.norm(name);
                    let repetition = if repeated { " This capture occurs inside a repetition; the surrounding repetition syntax controls how many fragments can match and how captures are reused in a transcriber." } else { "" };
                    result.push(ContractItem {
                        group: "Macro meta fragment".into(),
                        text: format!("In a macro_rules! matcher, {name}:meta introduces the metavariable {name} and matches an attribute's contents as syntax, such as inline or derive(Clone). In a matcher like #[{name}:meta], the surrounding #[ and ] are written separately; {name} captures the contents between them. A transcriber can later insert the captured syntax with {name} in a valid expansion position. Matching does not itself apply an attribute or run code; the expansion determines what is emitted.{repetition}"),
                        span: src.span(node), type_info: None,
                    });
                }
                continue;
            }
            pending.extend(named_children(node).into_iter().rev().map(|child| (child, repeated)));
        }
    }
    result
}
