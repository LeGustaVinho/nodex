//! Explain a simple positional `println!` format without assuming macro identity.
use nodex_ir::{Span, reading::ReadingContent};
use nodex_lang_api::{kit::truncate, syntax::Src};
use tree_sitter::Node;

fn ascii_ident(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        && text != "_"
}

/// Handle one ordinary string literal containing a single `{}` and one simple
/// identifier argument. Other formatting grammar needs its own explanation.
pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let name = node.child_by_field_name("macro")?;
    let path = src.norm(name);
    if path.rsplit("::").next() != Some("println") { return None; }
    let args = crate::expr::macro_args(src, node);
    if args.len() != 2 { return None; }
    let (format_start, format_end) = args[0];
    let (arg_start, arg_end) = args[1];
    if src.text[format_end..arg_start].trim() != "," { return None; }
    let format_written = &src.text[format_start..format_end];
    let inner = format_written.strip_prefix('"')?.strip_suffix('"')?;
    if inner.contains('\\') { return None; }
    let placeholder = inner.find("{}")?;
    if inner[..placeholder].contains(['{', '}']) || inner[placeholder + 2..].contains(['{', '}']) { return None; }
    let argument = &src.text[arg_start..arg_end];
    if !ascii_ident(argument) { return None; }
    let span = src.index.span(src.file, format_start + 1 + placeholder, format_start + 1 + placeholder + 2);
    Some((span, ReadingContent::Detail {
        label: "Format and print a value".into(),
        text: format!("If {}! resolves to Rust's standard println macro, {} is a format string and its first {{}} is replaced by the value of the following argument {} using that value's Display formatting. When the expanded code runs, println writes the formatted text followed by a newline to standard output. The argument's type must support the requested formatting. The spelling alone does not prove which macro this path resolves to; another macro named println could behave differently. Macro expansion occurs during compilation, not when the written invocation is parsed as an ordinary function call.", truncate(&path, 80), truncate(format_written, 100), argument),
        definition: None,
        borrow: false,
    }))
}
