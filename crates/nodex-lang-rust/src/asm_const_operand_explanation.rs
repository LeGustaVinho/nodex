//! Explain `const N` in a possible inline-assembly invocation.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return Vec::new(); }
    let Some(path) = node.child_by_field_name("macro") else { return Vec::new(); };
    let name = src.norm(path).replace(char::is_whitespace, "");
    if name.rsplit("::").next() != Some("asm") { return Vec::new(); }
    let Some(tree) = named_children(node).into_iter().find(|part| part.kind() == "token_tree") else { return Vec::new(); };
    if tree.has_error() { return Vec::new(); }
    let tokens: Vec<_> = children(tree).into_iter().filter(|part| !part.is_extra()).collect();
    if tokens.len() < 2 { return Vec::new(); }
    let mut result = Vec::new();
    for segment in tokens[1..tokens.len()-1].split(|part| part.kind() == ",").skip(1) {
        let operand = match segment { [name, eq, rest @ ..] if name.kind() == "identifier" && eq.kind() == "=" => rest, rest => rest };
        let [keyword, value] = operand else { continue; };
        if src.text(*keyword) != "const" || value.kind() != "identifier" { continue; }
        result.push((*keyword, ReadingContent::Detail {
            label: "Assembly constant operand".into(),
            text: format!("If this invocation resolves to Rust's standard `asm!`, `const {0}` supplies the compile-time value of the integer constant expression {0} for substitution into an assembly template placeholder. The value is inserted as text into the assembly, rather than passed in a register. The expression must satisfy the macro's constant and integer requirements; its actual value, the assembled instruction, and target validity cannot be determined from this syntax alone. Another macro named asm may interpret these tokens differently; inline assembly has safety obligations.", src.norm(*value)),
            definition: None, borrow: false,
        }));
    }
    result
}
