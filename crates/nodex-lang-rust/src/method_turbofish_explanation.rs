//! Explain explicit generic arguments on a method call.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

/// Recognize a method-call turbofish syntactically, without claiming which
/// method or generic parameters name resolution selects.
pub(crate) fn explain<'a>(src: Src<'_>, call: Node<'a>) -> Option<(Node<'a>, ReadingContent)> {
    if call.kind() != "call_expression" { return None; }
    let generic = call.child_by_field_name("function")?;
    if generic.kind() != "generic_function" { return None; }
    let field = generic.child_by_field_name("function")?;
    if field.kind() != "field_expression" { return None; }
    let receiver = field.child_by_field_name("value")?;
    let method = field.child_by_field_name("field")?;
    let arguments = generic.child_by_field_name("type_arguments")?;
    Some((arguments, ReadingContent::Detail {
        label: "Explicit method generic arguments".into(),
        text: format!("{} is the receiver and {} names the method called on it. The ::<...> notation supplies generic arguments {} explicitly to that method; the later parentheses contain the ordinary call arguments. The compiler checks the supplied arguments against the method's generic parameters and requirements. Their exact meaning and which method is selected depend on name resolution and type checking.", src.norm(receiver), src.norm(method), src.norm(arguments)),
        definition: None,
        borrow: false,
    }))
}
