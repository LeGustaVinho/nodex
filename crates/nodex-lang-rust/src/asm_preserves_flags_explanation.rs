//! Explain the preserves_flags option on an asm-like invocation.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let name = src.norm(node.child_by_field_name("macro")?);
    if name.rsplit("::").next()? != "asm" { return None; }
    let group = named_children(node).into_iter().find(|part| part.kind() == "token_tree")?;
    let tokens: Vec<_> = children(group).into_iter().filter(|part| !part.is_extra()).collect();
    if tokens.len() < 3 { return None; }
    for segment in tokens[1..tokens.len()-1].split(|part| part.kind() == ",").skip(1) {
        if segment.len() != 2 || src.text(segment[0]) != "options" || segment[1].kind() != "token_tree" { continue; }
        let options: Vec<_> = children(segment[1]).into_iter().filter(|part| !part.is_extra()).collect();
        if options.len() < 3 || options[0].kind() != "(" || options.last()?.kind() != ")" { continue; }
        let flag = options[1..options.len()-1].split(|part| part.kind() == ",").find_map(|option| {
            (option.len() == 1 && option[0].kind() == "identifier" && src.text(option[0]) == "preserves_flags").then(|| option[0])
        });
        if let Some(flag) = flag {
            return Some((flag, ReadingContent::Detail {
                label: "Possible preserved assembly flags".into(),
                text: format!("If {name}! resolves to Rust's standard inline assembly macro, `options(preserves_flags)` promises the compiler that the architecture-specific flags covered by this option have their original values when the assembly exits. Instructions may change them temporarily if they are restored before exit. The compiler can rely on that promise when optimizing. The assembly author must uphold it along with all other unsafe assembly rules; violating the contract can cause undefined behavior. Which flags are covered depends on the compilation target, and the source alone does not prove the instructions obey the contract. A different macro named asm may interpret these tokens differently."),
                definition: None, borrow: false,
            }));
        }
    }
    None
}
