//! Explain a function ABI written as a raw string literal.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_item" || node.child_by_field_name("body").is_none() { return None; }
    let modifiers = named_children(node).into_iter().find(|part| part.kind() == "function_modifiers")?;
    let external = named_children(modifiers).into_iter().find(|part| part.kind() == "extern_modifier")?;
    let written = src.text(external).strip_prefix("extern")?.trim();
    let after_r = written.strip_prefix('r')?;
    let hashes = after_r.chars().take_while(|c| *c == '#').count();
    let rest = after_r.get(hashes..)?.strip_prefix('"')?;
    let closing = format!("\"{}", "#".repeat(hashes));
    let name = rest.strip_suffix(&closing)?;
    let detail = if name == "C" { "The ABI name `C` selects the target platform's C calling convention, including how arguments and results are passed." } else { "The compiler determines whether this ABI name is supported and applies its corresponding calling convention for the target." };
    Some(ContractItem {
        group: "Raw string ABI on a function".into(),
        text: format!("`{}` uses the raw string literal `{written}` to write the ABI name `{name}`. The `r` and matching number of `#` delimiters define the literal; its contents are taken literally rather than interpreting backslash escapes. {detail} The braces still contain a Rust function body. The ABI spelling alone does not export an unmangled symbol, prove type compatibility with foreign code, or make a call safe; `unsafe` can impose additional caller obligations.", src.norm(external)),
        span: src.index.span(src.file, external.end_byte() - written.len(), external.end_byte()), type_info: None,
    })
}
