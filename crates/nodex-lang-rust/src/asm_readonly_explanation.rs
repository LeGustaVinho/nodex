//! Explain the `readonly` option of a possible standard `asm!` invocation.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    if src.norm(node.child_by_field_name("macro")?) != "asm" { return None; }
    let tree = named_children(node).into_iter().find(|child| child.kind() == "token_tree")?;
    let tokens: Vec<_> = children(tree).into_iter().filter(|part| !part.is_extra()).collect();
    if tokens.len() < 3 { return None; }
    let readonly = tokens[1..tokens.len() - 1].split(|part| part.kind() == ",").skip(1).find_map(|entry| {
        let [options, args] = entry else { return None; };
        if options.kind() != "identifier" || src.text(*options) != "options" || args.kind() != "token_tree" { return None; }
        let parts: Vec<_> = children(*args).into_iter().filter(|part| !part.is_extra()).collect();
        if parts.len() < 3 || parts.first()?.kind() != "(" || parts.last()?.kind() != ")" { return None; }
        parts[1..parts.len() - 1].split(|part| part.kind() == ",").find_map(|option| match option {
            [name] if name.kind() == "identifier" && src.text(*name) == "readonly" => Some(*name),
            _ => None,
        })
    })?;
    Some((readonly, ReadingContent::Detail {
        label: "Read-only assembly memory".into(),
        text: "If this `asm!` resolves to Rust's standard inline assembly macro, `options(readonly)` promises that the assembly does not write memory accessible outside the assembly; it may read memory. Memory private to the assembly, such as stack space allocated within it, is exempt from this memory restriction. The compiler also assumes the assembly performs no synchronization with other threads; it may optimize based on these promises. The author must uphold them: violating them is undefined behavior. This option does not itself make the assembly pure or safe, check its instructions, or remove the other unsafe obligations. The spelling alone does not prove that `asm!` resolves to the standard macro.".into(),
        definition: None, borrow: false,
    }))
}
