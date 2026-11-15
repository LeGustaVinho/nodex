//! Explain a late register output in a possible standard `asm!` invocation.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let macro_name = node.child_by_field_name("macro")?;
    if src.norm(macro_name) != "asm" { return None; }
    let tree = named_children(node).into_iter().find(|child| child.kind() == "token_tree")?;
    let tokens: Vec<_> = children(tree).into_iter().filter(|part| !part.is_extra()).collect();
    if tokens.len() < 3 { return None; }
    let operand = tokens[1..tokens.len() - 1].split(|part| part.kind() == ",").skip(1).find_map(|entry| {
        let entry = match entry {
            [name, equal, rest @ ..] if name.kind() == "identifier" && equal.kind() == "=" => rest,
            rest => rest,
        };
        let [lateout, reg, target] = entry else { return None; };
        if lateout.kind() != "identifier" || src.text(*lateout) != "lateout" || reg.kind() != "token_tree" || target.kind() != "identifier" { return None; }
        let register: Vec<_> = children(*reg).into_iter().filter(|part| !part.is_extra()).collect();
        if !matches!(register.as_slice(), [open, class, close] if open.kind() == "(" && class.kind() == "identifier" && src.text(*class) == "reg" && close.kind() == ")") { return None; }
        Some((*lateout, *target))
    })?;
    Some((operand.0, ReadingContent::Detail {
        label: "Late assembly register output".into(),
        text: format!("If this `asm!` resolves to Rust's standard inline assembly macro, `lateout(reg) {}` declares an output: the compiler chooses a register from the target's `reg` class and writes the assembly's final register value into {}. `lateout` allows that register to be shared with an input, so the assembly must read all inputs before writing this output; otherwise the output may overwrite an input too early. The target determines the available registers and supported operand types. The macro name alone does not prove resolution to the standard macro, and this description does not verify the assembly or its safety requirements.", src.text(operand.1), src.text(operand.1)),
        definition: None, borrow: false,
    }))
}
