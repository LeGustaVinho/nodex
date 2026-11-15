//! Explain unstable C variadic function definitions.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_item" || node.child_by_field_name("body").is_none() || node.has_error() { return None; }
    let modifiers = named_children(node).into_iter().find(|part| part.kind() == "function_modifiers")?;
    if !children(modifiers).iter().any(|part| part.kind() == "unsafe") { return None; }
    let external = named_children(modifiers).into_iter().find(|part| part.kind() == "extern_modifier")?;
    let abi = named_children(external).into_iter().find(|part| part.kind() == "string_literal")?;
    if src.text(abi) != "\"C\"" { return None; }
    let parameters = node.child_by_field_name("parameters")?;
    let args = named_children(parameters);
    let variadic = args.last().filter(|part| part.kind() == "variadic_parameter" && !part.has_error())?;
    let dots = children(*variadic).into_iter().find(|part| part.kind() == "...").unwrap_or(*variadic);
    Some(ContractItem {
        group: "Variadic C function definition".into(),
        text: "This is a Rust function body using the C calling convention. Its final `...` accepts zero or more extra arguments after any fixed parameters; a name such as `args` binds access to those arguments inside the body. Their number and types are not specified by `...`: the implementation and callers must agree on their format and C argument promotions. Reading an absent argument or using the wrong type can cause undefined behavior. `unsafe` means callers must uphold the function's safety requirements. Defining C-variadic functions uses the unstable `c_variadic` feature and requires a compiler/toolchain that supports it; this syntax alone does not establish that the function is exported or that any particular call is valid.".into(),
        span: src.span(dots), type_info: None,
    })
}
