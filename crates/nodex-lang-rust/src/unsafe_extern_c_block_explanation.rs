//! Explain unsafe external blocks using the target's C ABI.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn block_detail(src: Src<'_>, block: Node<'_>) -> Option<ContractItem> {
    if block.kind() != "foreign_mod_item" || block.has_error() { return None; }
    let body = block.child_by_field_name("body")?;
    if !children(block).iter().any(|part| part.kind() == "unsafe") { return None; }
    let modifier = named_children(block).into_iter().find(|part| part.kind() == "extern_modifier")?;
    let abi = named_children(modifier).into_iter().find(|part| part.kind() == "string_literal")?;
    if src.text(abi) != "\"C\"" { return None; }
    Some(ContractItem {
        group: "Unsafe C ABI external block".into(),
        text: "`unsafe extern \"C\" { ... }` declares foreign functions and statics available in this module or block; it does not define their bodies or call them. `\"C\"` specifies the target's C calling convention, including how arguments and results are passed. The author is responsible for the declarations matching the actual external items, including their ABI and types; the compiler cannot establish that from this block alone. In such a block foreign functions are unsafe to call by default unless explicitly declared `safe`; `unsafe` on the block does not itself make a call safe. In the 2024 edition the block requires `unsafe`; earlier editions permitted omitting it. Successful linking and correct runtime behavior depend on the external definitions and target.".into(),
        span: src.header_span(block, Some(body)), type_info: None,
    })
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "function_signature_item" {
        return node.parent().and_then(|body| body.parent()).and_then(|block| block_detail(src, block)).into_iter().collect();
    }
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    named_children(container).into_iter().filter_map(|part| block_detail(src, part)).collect()
}
