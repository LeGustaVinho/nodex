//! Explain mixed inferred and literal constant arguments in a turbofish.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if !matches!(node.kind(), "generic_type" | "generic_function") || node.has_error() { return None; }
    let args = node.child_by_field_name("type_arguments")?;
    if !src.text(node).contains("::") { return None; }
    let values = named_children(args);
    if values.len() != 2 || src.text(values[0]) != "_" || values[1].kind() != "integer_literal" { return None; }
    let comma = children(args).into_iter().find(|part| part.kind() == ",")?;
    let owner = std::iter::successors(node.parent(), Node::parent).find(|p| matches!(p.kind(), "call_expression" | "let_declaration" | "expression_statement"))?;
    let value = src.norm(values[1]);
    Some((owner, comma, ReadingContent::Detail {
        label: "Inferred and constant generic arguments".into(),
        text: format!("`::<_, {value}>` supplies two generic arguments in order. The underscore asks the compiler to infer the first argument from context; depending on the selected parameter and what Rust permits, it may represent a type or an inferred constant. The second argument is the integer constant {value}, not an ordinary runtime call argument. For a declaration whose first parameter is a type and second is a compatible const parameter, this chooses that inferred type and the constant value {value}. Name resolution and the declaration determine the parameters; the compiler checks inference, parameter types, and whether this syntax is allowed in the context."),
        definition: None, borrow: false,
    }))
}
