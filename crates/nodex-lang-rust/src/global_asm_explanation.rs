//! Explain a module-level `global_asm!` invocation without assuming resolution.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::{kit::truncate, syntax::{named_children, Src}};
use tree_sitter::Node;

fn detail(src: Src<'_>, invocation: Node<'_>) -> Option<ContractItem> {
    if invocation.kind() != "macro_invocation" || invocation.has_error() { return None; }
    let path = invocation.child_by_field_name("macro")?;
    let compact: String = src.text(path).chars().filter(|ch| !ch.is_whitespace()).collect();
    if compact.rsplit("::").next()? != "global_asm" { return None; }
    let name = if path.kind() == "scoped_identifier" { path.child_by_field_name("name").unwrap_or(path) } else { path };
    let input = named_children(invocation).into_iter().find(|part| part.kind() == "token_tree")?;
    Some(ContractItem {
        group: "Global assembly macro".into(),
        text: format!("If `{}!` resolves to Rust's built-in global_asm macro, its assembly templates are incorporated into the compiler's assembly output at global scope, outside a function. They can contain labels, instructions, or assembler directives; the invocation itself is expanded during compilation, not executed like a runtime function call. Global assembly can use `const` and `sym` operands rather than function-local register operands. The assembly syntax and resulting behavior depend on the target, assembler, options, and provided code; the author must ensure symbols, calling conventions, and safety requirements are correct. This written invocation and its input {} do not prove which macro resolves, whether configuration includes it, or whether it assembles and links successfully.", truncate(&src.norm(path), 100), truncate(&src.norm(input), 100)),
        span: src.span(name), type_info: None,
    })
}

/// Module-level invocations have no flow of their own; attach them to the containing module.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    named_children(container).into_iter().filter_map(|part| {
        let invocation = if part.kind() == "macro_invocation" { Some(part) }
            else if part.kind() == "expression_statement" { named_children(part).into_iter().find(|child| child.kind() == "macro_invocation") }
            else { None }?;
        detail(src, invocation)
    }).collect()
}
