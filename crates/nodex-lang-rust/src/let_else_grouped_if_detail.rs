//! Explain grouping a brace-ending `if` initializer in a let-else statement.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" || node.has_error() { return None; }
    node.child_by_field_name("alternative")?;
    let value = node.child_by_field_name("value")?;
    if value.kind() != "parenthesized_expression" { return None; }
    let inner = named_children(value);
    let [conditional] = inner.as_slice() else { return None; };
    if conditional.kind() != "if_expression" || conditional.child_by_field_name("alternative").is_none() { return None; }
    let opening = children(value).into_iter().find(|child| child.kind() == "(")?;
    let pattern = node.child_by_field_name("pattern")?;
    Some((opening, ReadingContent::Detail {
        label: "Group an if initializer for let-else".into(),
        text: format!("In this let-else, the parenthesized `if` is the initializer for pattern `{}`. Rust's let-else syntax does not permit the initializer to end directly in `}}` before its `else`; grouping the `if ... else ...` in parentheses makes the initializer end in `)` instead. The if condition selects one branch's value first, then the pattern is tested against that value. A successful match introduces the pattern's bindings for subsequent code; a failed match executes the outer else block, which must diverge, for example by returning. The inner `else` belongs to the if; the later `else` belongs to let-else. Pattern names still require normal resolution.", src.norm(pattern)),
        definition: None, borrow: false,
    }))
}
