//! Explain a recoverable `unsafe mod` syntax error without treating it as valid Rust.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "mod_item" { return None; }
    let previous = node.prev_sibling()?;
    if previous.kind() != "ERROR" || src.text(previous) != "unsafe" { return None; }
    let gap = src.text.get(previous.end_byte()..node.start_byte())?;
    if !gap.chars().all(char::is_whitespace) { return None; }
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "Invalid unsafe module declaration".into(),
        text: format!("`unsafe mod {}` is not a valid Rust module declaration: `unsafe` cannot qualify `mod`. A module groups items and does not establish an unsafe context for everything inside it. To declare the module, remove `unsafe`; individual operations that require it must still use an appropriate unsafe block or other supported unsafe construct and satisfy its safety requirements. The parser recovered the module after an error, so this explanation identifies the written syntax rather than claiming the program compiles.", src.norm(name)),
        span: src.span(previous), type_info: None,
    })
}
