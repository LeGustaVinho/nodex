//! Explain the parenthesized form of a Rust macro invocation.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let name = node.child_by_field_name("macro")?;
    let bang = children(node).into_iter().find(|child| child.kind() == "!")?;
    let tokens = named_children(node).into_iter().find(|child| child.kind() == "token_tree")?;
    let open = children(tokens).into_iter().find(|child| !child.is_extra())?;
    if open.kind() != "(" { return None; }
    Some((bang, ReadingContent::Detail {
        label: "Invoke a macro with parentheses".into(),
        text: format!("`{}!` invokes a macro; the exclamation mark is part of the invocation, not logical negation or an ordinary function call. The parentheses delimit the tokens supplied to the macro. During compilation the macro is expanded into Rust syntax, whose meaning depends on the macro definition and this position. The input is tokens: an apparent argument such as `value` is not necessarily evaluated, and neither evaluation count nor order can be inferred from this spelling alone. Rust also has bracket and brace delimiters, but a macro's accepted syntax and behavior depend on its definition.", src.norm(name)),
        definition: None, borrow: false,
    }))
}
