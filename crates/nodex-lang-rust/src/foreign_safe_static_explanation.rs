//! Explain safe immutable foreign statics on the containing module's contract.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

/// The current parser repairs unsupported `safe static` by blanking `safe` in
/// an equal-length copy. Recover only a whole, adjacent original keyword and
/// reject occurrences in comments; byte offsets remain those of the source.
fn safe_keyword(src: Src<'_>, body: Node<'_>, item: Node<'_>) -> Option<usize> {
    let keyword = children(item).into_iter().find(|part| part.kind() == "static")?;
    let prefix = src.text.get(..keyword.start_byte())?.trim_end();
    if !prefix.ends_with("safe") { return None; }
    let start = prefix.len().checked_sub(4)?;
    if start < body.start_byte() { return None; }
    if prefix.get(..start)?.chars().next_back().is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '#')) { return None; }
    let mut at = body.descendant_for_byte_range(start, start + 4);
    while let Some(node) = at {
        if matches!(node.kind(), "line_comment" | "block_comment" | "string_literal" | "raw_string_literal") { return None; }
        if node == body { break; }
        at = node.parent();
    }
    Some(start)
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => node,
        "mod_item" => match node.child_by_field_name("body") { Some(body) => body, None => return Vec::new() },
        _ => return Vec::new(),
    };
    let mut result = Vec::new();
    for external in named_children(container).into_iter().filter(|part| part.kind() == "foreign_mod_item") {
        if !children(external).iter().any(|part| part.kind() == "unsafe") { continue; }
        let Some(modifier) = named_children(external).into_iter().find(|part| part.kind() == "extern_modifier") else { continue };
        if !children(modifier).iter().any(|part| part.kind() == "string_literal" && src.text(*part) == "\"C\"") { continue; }
        let Some(body) = external.child_by_field_name("body") else { continue };
        for declaration in named_children(body).into_iter().filter(|part| part.kind() == "static_item" && !part.has_error()) {
            if declaration.child_by_field_name("value").is_some() || children(declaration).iter().any(|part| part.kind() == "mutable_specifier") { continue; }
            let (Some(name), Some(ty)) = (declaration.child_by_field_name("name"), declaration.child_by_field_name("type")) else { continue };
            let Some(safe_start) = safe_keyword(src, body, declaration) else { continue };
            let symbol = src.norm(name);
            let native_type = src.norm(ty);
            result.push(ContractItem {
                group: "Safe foreign static".into(),
                text: format!("This unsafe extern \"C\" block declares {symbol} as a foreign static of type {native_type}; it does not define or initialize that value here. The linker must resolve the external symbol. The author of the extern block is responsible for a correct declaration and for the foreign definition meeting Rust's validity and safety requirements. The explicit safe qualifier permits accessing this immutable foreign static without an unsafe block for the foreign-static access itself; without safe, a foreign static is unsafe to access by default. An immutable foreign static must be initialized before Rust code runs and must not be mutated while it runs, except through valid UnsafeCell interior mutability. This source alone does not prove that the symbol exists, has the declared type, or satisfies those requirements. A mutable static has separate unsafe access rules even if marked safe."),
                span: src.index.span(src.file, safe_start, safe_start + 4), type_info: None,
            });
        }
    }
    result
}
