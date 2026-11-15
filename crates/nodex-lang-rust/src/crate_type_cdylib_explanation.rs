//! Explain a written crate-level request for a native dynamic library.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn is_cdylib_literal(src: Src<'_>, value: Node<'_>) -> bool {
    let text = src.text(value);
    if value.kind() == "string_literal" { return text == "\"cdylib\""; }
    if value.kind() != "raw_string_literal" { return false; }
    let Some(rest) = text.strip_prefix('r') else { return false; };
    let hashes = rest.chars().take_while(|ch| *ch == '#').count();
    let Some(body) = rest.get(hashes..) else { return false; };
    body == format!("\"cdylib\"{}", "#".repeat(hashes))
}

/// Attach the explanation to the file's module symbol, without treating
/// nested modules' attributes as if they belonged to this source file.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "source_file" { return Vec::new(); }
    named_children(node).into_iter().filter_map(|part| {
        if part.kind() != "inner_attribute_item" || part.has_error() { return None; }
        let attribute = named_children(part).into_iter().find(|child| child.kind() == "attribute")?;
        let path = named_children(attribute).into_iter().next()?;
        if src.text(path) != "crate_type" { return None; }
        let value = attribute.child_by_field_name("value")?;
        if !is_cdylib_literal(src, value) { return None; }
        Some(ContractItem {
            group: "Native dynamic library crate type".into(),
            text: "`#![crate_type = \"cdylib\"]` is an inner attribute requesting a native dynamic-library artifact when accepted at the crate root. A cdylib is intended to be loaded from another language; its file format and extension depend on the target platform. The attribute does not automatically make every Rust function a compatible C entry point: exported symbols, ABI, types and safety must be arranged separately. `rustc --crate-type` overrides crate_type attributes, and Cargo or other build tools can choose the compiler arguments, so this source alone does not prove which artifact a build produces.".into(),
            span: src.span(part), type_info: None,
        })
    }).collect()
}
