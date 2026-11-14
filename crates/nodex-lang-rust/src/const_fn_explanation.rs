//! Explain `const fn` declarations without promising all calls run at compile time.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return None; }
    let modifiers = children(node).into_iter().find(|child| child.kind() == "function_modifiers")?;
    let keyword = children(modifiers).into_iter().find(|child| child.kind() == "const")?;
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "Const function".into(),
        text: format!("const fn declares {} as a function eligible for evaluation in a constant context, such as a const initializer, when the call and arguments satisfy the compiler's constant-evaluation rules. Rust checks the function body against restrictions on operations allowed in const functions. A call outside a constant context behaves as an ordinary call and may execute at runtime; writing const fn does not guarantee that every call is evaluated during compilation. Constant evaluation can fail, for example when an evaluated operation panics.", src.norm(name)),
        span: src.span(keyword),
        type_info: None,
    })
}
