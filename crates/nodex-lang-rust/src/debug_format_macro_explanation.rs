//! Explain the specific `format!("{:?}", value)` spelling without guessing the macro identity.
use nodex_ir::{Span, reading::ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let path = node.child_by_field_name("macro")?;
    let name = src.norm(path);
    if name.rsplit("::").next()? != "format" { return None; }
    let group = named_children(node).into_iter().find(|part| part.kind() == "token_tree")?;
    let tokens: Vec<_> = children(group).into_iter().filter(|part| !part.is_extra()).collect();
    if tokens.len() != 5 && tokens.len() != 6 { return None; }
    if tokens[0].kind() != "(" || tokens[1].kind() != "string_literal" || src.text(tokens[1]) != "\"{:?}\""
        || tokens[2].kind() != "," || tokens[3].kind() != "identifier" || tokens.last()?.kind() != ")"
        || (tokens.len() == 6 && tokens[4].kind() != ",") { return None; }
    let marker_start = tokens[1].start_byte() + 3; // ? in the exact literal "{:?}"
    let marker = src.index.span(src.file, marker_start, marker_start + 1);
    let argument = src.norm(tokens[3]);
    Some((marker, ReadingContent::Detail {
        label: "Possible Debug formatting".into(),
        text: format!("If {}! resolves to Rust's standard format! macro, it creates and returns a new owned String instead of printing it. The format string {{:?}} is a placeholder requesting the Debug representation of {}, so its type must support the standard Debug formatting trait. The representation is intended for debugging and is not guaranteed to be stable or machine-readable. Macro expansion happens during compilation; formatting occurs when the resulting code runs. A locally defined or imported macro can have the same name, so the spelling alone does not prove that these standard semantics apply.", name, argument),
        definition: None,
        borrow: false,
    }))
}
