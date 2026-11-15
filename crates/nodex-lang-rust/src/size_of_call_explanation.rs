//! Explain an explicitly typed call to a possible standard `size_of` function.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "call_expression" || node.has_error() { return None; }
    let generic = node.child_by_field_name("function").filter(|part| part.kind() == "generic_function")?;
    let function = generic.child_by_field_name("function")?;
    let path = src.norm(function).replace(char::is_whitespace, "");
    if !matches!(path.as_str(), "std::mem::size_of" | "::std::mem::size_of" | "core::mem::size_of" | "::core::mem::size_of") { return None; }
    let args = generic.child_by_field_name("type_arguments")?;
    let types = named_children(args);
    let [ty] = types.as_slice() else { return None; };
    let arguments = node.child_by_field_name("arguments")?;
    if !named_children(arguments).is_empty() { return None; }
    Some((function, ReadingContent::Detail {
        label: "Size of a type".into(),
        text: format!("If {path} resolves to Rust's standard library function, `size_of::<{}>()` returns a `usize` count of bytes occupied by one value of that type, including any padding needed for its layout. The type is supplied between `<` and `>`; the call takes no runtime value. A zero-sized type has size 0, so multiple such values need not occupy separate bytes. The actual size and layout can depend on the type, target, and representation; this syntax alone does not calculate a number or establish name resolution.", src.norm(*ty)),
        definition: None, borrow: false,
    }))
}

