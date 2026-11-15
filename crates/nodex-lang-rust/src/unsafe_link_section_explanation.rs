//! Explain the unsafe `link_section` attribute while preserving its written string literal.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn section_literal<'t>(src: Src<'_>, item: Node<'t>) -> Option<Node<'t>> {
    if item.kind() != "attribute_item" || item.has_error() { return None; }
    let attr = named_children(item).into_iter().find(|part| part.kind() == "attribute")?;
    let name = named_children(attr).into_iter().find(|part| src.text(*part) == "unsafe")?;
    // The unsafe wrapper has one direct link_section name/value argument.
    if !matches!(name.kind(), "identifier" | "unsafe") { return None; }
    let args = attr.child_by_field_name("arguments")?;
    let tokens: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
    if tokens.len() != 5 || tokens[0].kind() != "(" || tokens[1].kind() != "identifier"
        || src.text(tokens[1]) != "link_section" || tokens[2].kind() != "=" || tokens[4].kind() != ")" { return None; }
    let literal = tokens[3];
    if !matches!(literal.kind(), "string_literal" | "raw_string_literal") { return None; }
    let text = src.text(literal);
    if !(text.starts_with('"') || text.starts_with('r')) { return None; }
    Some(literal)
}

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "static_item") { return Vec::new(); }
    // Foreign declarations do not define code or static contents in this file.
    if declaration.parent().and_then(|body| body.parent()).is_some_and(|owner| owner.kind() == "foreign_mod_item") { return Vec::new(); }
    let mut previous = declaration.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = attribute.prev_named_sibling();
        let Some(literal) = section_literal(src, attribute) else { continue };
        result.push(ContractItem {
            group: "Unsafe object-file section".into(),
            text: format!("This unsafe link_section attribute specifies the object-file section for this function's code or static's contents. The section is written as {}; Rust processes string-literal escapes when determining its name. Section names and their meaning depend on the target, object format, linker, and runtime. unsafe(...) marks an obligation the author must uphold: for example, placing mutable data in a read-only section can cause undefined behavior. The wrapper does not prove that the placement is safe. The final linker can affect placement or retention, and the attribute does not by itself define a calling convention or guarantee a symbol remains in the final binary. If link_section is repeated on one item, only the first takes effect. Rust edition 2024 requires the unsafe wrapper; earlier editions also permit the unwrapped spelling.", src.text(literal)),
            span: src.span(literal), type_info: None,
        });
    }
    result
}
