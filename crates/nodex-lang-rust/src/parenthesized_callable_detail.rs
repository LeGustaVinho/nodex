//! Explain calling a parenthesized expression and callable fields.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "call_expression" { return None; }
    let callee = node.child_by_field_name("function")?;
    if callee.kind() != "parenthesized_expression" { return None; }
    let inner = named_children(callee).into_iter().find(|part| !part.is_extra())?;
    let expression = src.norm(inner);
    let field_detail = if inner.kind() == "field_expression" {
        format!(" Here `{}` selects a field value and the outer call invokes that value; `obj.field(args)` instead uses method call syntax. The parentheses make this distinction explicit, provided the selected field exists and is callable.", src.norm(callee))
    } else {
        " This grouping can also distinguish a callable field from method-call syntax: `(obj.field)(args)` calls the field value, while `obj.field(args)` is a method call.".into()
    };
    Some((callee, ReadingContent::Detail {
        label: "Call a grouped expression".into(),
        text: format!("The first parentheses group the expression `{expression}`; the following parentheses supply arguments to call the resulting value. Grouping does not itself call the expression or create a tuple. The value must be callable with those arguments, for example a function pointer or a compatible closure.{field_detail} The spelling alone does not resolve its type, target, argument compatibility, or whether calling it moves or borrows the callable."),
        definition: None, borrow: false,
    }))
}
