//! Explain the explicit crate-type attribute on a source-file scope.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "source_file" { return Vec::new(); }
    named_children(node).into_iter().filter_map(|part| {
        if part.kind() != "inner_attribute_item" || part.has_error() { return None; }
        let attribute = named_children(part).into_iter().find(|child| child.kind() == "attribute")?;
        let name = named_children(attribute).into_iter().next()?;
        if src.norm(name) != "crate_type" { return None; }
        let value = attribute.child_by_field_name("value")?;
        if value.kind() != "string_literal" || src.text(value) != "\"rlib\"" { return None; }
        Some(ContractItem {
            group: "Rust library crate type".into(),
            text: "`#![crate_type = \"rlib\"]` is an inner crate attribute requesting a Rust library artifact when this file is used as the crate root. An rlib is a compiler-readable library artifact containing Rust metadata that rustc can use in later compilation and linking; it is not a runnable executable. It differs from `staticlib`, which targets linking into non-Rust applications. Multiple crate_type attributes can request multiple outputs, but if one or more `--crate-type` command-line flags are supplied, rustc uses the command-line choices instead of these attributes. The actual crate root, build options and emitted artifacts are not established by this syntax alone; this attribute is build configuration, not code executed by the program.".into(),
            span: src.span(part), type_info: None,
        })
    }).collect()
}
