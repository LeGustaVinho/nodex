//! Explain interpreter-style shebangs on source file modules.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

/// Recognize an absolute interpreter path after `#!` at the file start.
/// This deliberately avoids guessing whether attribute-like or comment-prefixed
/// forms are shebangs: the Rust lexer has additional rules for those cases.
pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "source_file" { return None; }
    let start = if src.text.starts_with('\u{feff}') { '\u{feff}'.len_utf8() } else { 0 };
    let rest = src.text.get(start..)?;
    let after_marker = rest.strip_prefix("#!")?;
    if !after_marker.trim_start_matches([' ', '\t']).starts_with('/') { return None; }
    let end = start + rest.find('\n').unwrap_or(rest.len());
    let end = if src.text.as_bytes().get(end.wrapping_sub(1)) == Some(&b'\r') { end - 1 } else { end };
    Some(ContractItem {
        group: "Shebang".into(),
        text: "This first line is a shebang: on systems that support it, launching an executable script can use the path after `#!` to choose an interpreter. For `/usr/bin/env`, the following name is looked up by env; actual execution depends on the operating system, permissions and the interpreter being available. Rust removes a recognized shebang before parsing the source, so this line is not a Rust instruction and does not itself run or compile the program. It may appear at the start of the file after an optional byte-order mark; a `#![...]` inner attribute is a different Rust construct.".into(),
        span: src.index.span(src.file, start, end),
        type_info: None,
    })
}
