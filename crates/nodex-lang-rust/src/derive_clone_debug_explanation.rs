//! Explain `#[derive(Clone, Debug)]` on a type declaration.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "struct_item" | "enum_item" | "union_item") { return Vec::new(); }
    let Some(name) = node.child_by_field_name("name") else { return Vec::new(); };
    let mut output = Vec::new();
    let mut previous = node.prev_named_sibling();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = attribute.prev_named_sibling();
        if attribute.kind() != "attribute_item" { continue; }
        let compact: String = src.text(attribute).chars().filter(|ch| !ch.is_whitespace()).collect();
        let Some(args) = compact.strip_prefix("#[derive(").and_then(|s| s.strip_suffix(")]")) else { continue; };
        let derives: Vec<_> = args.split(',').map(|part| part.rsplit("::").next().unwrap_or(part)).collect();
        if !derives.contains(&"Clone") || !derives.contains(&"Debug") { continue; }
        output.push(ContractItem {
            group: "Derive Clone and Debug".into(),
            text: format!("`#[derive(Clone, Debug)]` is an attribute on {}. It asks the compiler to apply the listed derive macros to this declaration and add the implementations they generate. If Clone and Debug resolve to the standard derives, the result includes Clone (explicit duplication via `.clone()`) and Debug (developer-oriented formatting such as `{{:?}}`). These derives may impose requirements on fields or type parameters; they do not automatically call clone or print anything. The written names alone do not prove which derive macros they resolve to.", src.norm(name)),
            span: src.span(attribute), type_info: None,
        });
    }
    output
}
