//! Explain an inner `debugger_visualizer` Natvis-file attribute without assuming the target.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn natvis_literal<'t>(src: Src<'_>, item: Node<'t>) -> Option<Node<'t>> {
    if item.kind() != "inner_attribute_item" || item.has_error() { return None; }
    let attr = named_children(item).into_iter().find(|part| part.kind() == "attribute")?;
    let name = named_children(attr).into_iter().find(|part| part.kind() == "identifier")?;
    if src.text(name) != "debugger_visualizer" { return None; }
    let args = attr.child_by_field_name("arguments")?;
    let tokens: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
    if tokens.len() != 5 && tokens.len() != 6 { return None; }
    if tokens[0].kind() != "(" || tokens[1].kind() != "identifier" || src.text(tokens[1]) != "natvis_file"
        || tokens[2].kind() != "=" || tokens.last()?.kind() != ")" || (tokens.len() == 6 && tokens[4].kind() != ",") { return None; }
    let literal = tokens[3];
    if !matches!(literal.kind(), "string_literal" | "raw_string_literal") { return None; }
    let written = src.text(literal);
    (written.starts_with('"') || written.starts_with('r')).then_some(literal)
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let container = match node.kind() {
        "source_file" => node,
        "mod_item" => match node.child_by_field_name("body") { Some(body) => body, None => return Vec::new() },
        _ => return Vec::new(),
    };
    named_children(container).into_iter().filter_map(|item| {
        let path = natvis_literal(src, item)?;
        Some(ContractItem {
            group: "Natvis debugger visualizer".into(),
            text: format!("This inner #![debugger_visualizer(...)] attribute belongs to the enclosing crate or module. natvis_file names a Natvis XML visualizer, written here as {}; Rust interprets escapes in an ordinary string but preserves the contents of a raw string; the resulting path is relative to the source file containing the attribute. On supported -windows-msvc targets, the compiler can embed the visualizer in debugging information so a compatible Microsoft debugger can display values using its rules. The source spelling alone does not establish that the file exists, that debug information is present, that the selected target supports Natvis embedding, or that a particular debugger uses it. The attribute configures debugging information; it does not execute the visualizer as Rust code or change the program's ordinary computation. Multiple debugger_visualizer attributes can specify multiple files.", src.text(path)),
            span: src.span(path), type_info: None,
        })
    }).collect()
}
