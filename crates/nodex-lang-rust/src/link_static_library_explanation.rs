//! Explain native static-library selection on an extern block.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn attributes_for_extern(src: Src<'_>, external: Node<'_>) -> Vec<ContractItem> {
    if external.kind() != "foreign_mod_item" { return Vec::new(); }
    let mut previous = external.prev_named_sibling();
    let mut items = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = attribute.prev_named_sibling();
        if attribute.kind() != "attribute_item" || attribute.has_error() { continue; }
        let Some(meta) = named_children(attribute).into_iter().find(|part| part.kind() == "attribute") else { continue; };
        let Some(name) = named_children(meta).into_iter().find(|part| part.kind() == "identifier") else { continue; };
        if src.text(name) != "link" { continue; }
        let Some(args) = meta.child_by_field_name("arguments") else { continue; };
        let tokens: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
        let mut library = None;
        let mut kind_static = false;
        for window in tokens.windows(3) {
            let (key, equal, value) = (window[0], window[1], window[2]);
            if key.kind() != "identifier" || equal.kind() != "=" || value.kind() != "string_literal" { continue; }
            match src.text(key) {
                "name" => library = Some(value),
                "kind" if src.text(value) == "\"static\"" => kind_static = true,
                _ => {},
            }
        }
        let Some(library) = library.filter(|_| kind_static) else { continue; };
        items.push(ContractItem {
            group: "Native static library link".into(),
            text: format!("This #[link] attribute applies to the following extern block. name = {} specifies the native library name as written; kind = \"static\" requests a static native library for linking. The target toolchain and build configuration determine how to locate and link that library, so this source alone does not prove that it was found or linked successfully. The extern block declares foreign items; this attribute does not implement them or by itself specify their calling convention. Here static describes the library linkage, not a Rust variable's mutability or lifetime.", src.text(library)),
            span: src.span(attribute),
            type_info: None,
        });
    }
    items.reverse();
    items
}

/// Show the explanation on each foreign declaration and the enclosing module, including empty blocks.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "foreign_mod_item" { return attributes_for_extern(src, node); }
    if let Some(external) = node.parent().and_then(|body| body.parent()).filter(|parent| parent.kind() == "foreign_mod_item") {
        return attributes_for_extern(src, external);
    }
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    named_children(container).into_iter().filter(|item| item.kind() == "foreign_mod_item")
        .flat_map(|external| attributes_for_extern(src, external)).collect()
}
