//! Explain the single-place inout register operand in an asm-like invocation.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let path = node.child_by_field_name("macro")?;
    let name = src.norm(path);
    if name.rsplit("::").next()? != "asm" { return None; }
    let group = named_children(node).into_iter().find(|part| part.kind() == "token_tree")?;
    let tokens: Vec<_> = children(group).into_iter().filter(|part| !part.is_extra()).collect();
    if tokens.len() < 3 { return None; }
    for segment in tokens[1..tokens.len()-1].split(|part| part.kind() == ",").skip(1) {
        let parts = if segment.len() >= 2 && segment[0].kind() == "identifier" && segment[1].kind() == "=" { &segment[2..] } else { segment };
        if parts.len() != 3 || src.text(parts[0]) != "inout" || parts[1].kind() != "token_tree" || parts[2].kind() != "identifier" { continue; }
        let register: Vec<_> = children(parts[1]).into_iter().filter(|part| !part.is_extra()).collect();
        if register.len() != 3 || register[0].kind() != "(" || src.text(register[1]) != "reg" || register[2].kind() != ")" { continue; }
        let place = src.norm(parts[2]);
        return Some((parts[0], ReadingContent::Detail {
            label: "Possible assembly input and output".into(),
            text: format!("If {name}! resolves to Rust's standard inline assembly macro, `inout(reg) {place}` supplies the initial value of {place} in a register selected from the target's general register class and writes that register's final value back to the same place after the assembly. The place must be suitable for both reading and writing. Unlike `inout(reg) input => output`, this form uses one place for both roles. The assembly author must obey the operand and safety contracts; the selected register, supported types, and instruction behavior depend on the target. A different macro named asm may interpret these tokens differently, and this source alone does not prove macro resolution or assembly correctness."),
            definition: None, borrow: false,
        }));
    }
    None
}
