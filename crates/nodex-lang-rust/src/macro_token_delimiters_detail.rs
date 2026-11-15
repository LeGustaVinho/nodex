//! Explain nested, balanced token-tree groups in macro invocations.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(_src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return Vec::new(); }
    let Some(tree) = named_children(node).into_iter().find(|part| part.kind() == "token_tree") else { return Vec::new() };
    let mut pending = vec![tree];
    let mut result = Vec::new();
    while let Some(group) = pending.pop() {
        let parts: Vec<_> = children(group).into_iter().filter(|part| !part.is_extra()).collect();
        let (Some(open), Some(close)) = (parts.first(), parts.last()) else { continue };
        let pair = match (open.kind(), close.kind()) {
            ("(", ")") => "`(` and `)`",
            ("[", "]") => "`[` and `]`",
            ("{", "}") => "`{` and `}`",
            _ => continue,
        };
        result.push((*open, ReadingContent::Detail {
            label: "Macro token-tree delimiters".into(),
            text: format!("The matching delimiters {pair} enclose a group of macro input tokens. Rust token trees can use three paired forms: `(...)`, `[...]`, and `{{...}}`; a group can contain tokens and further nested groups. These delimiters organize the input given to the macro, not necessarily an evaluated expression, an array, or a code block. Macro matching and expansion determine how the contents are interpreted; the written tokens alone do not prove the resolved macro or its behavior. The matching rules for outer macro delimiters and nested delimiters can differ, so these forms are not universally interchangeable."),
            definition: None, borrow: false,
        }));
        pending.extend(parts.into_iter().filter(|part| part.kind() == "token_tree").rev());
    }
    result
}
