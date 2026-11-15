//! Explain braced Unicode scalar escapes in ordinary UTF-8 string literals.
use nodex_ir::{reading::ReadingContent, Span};
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

/// Scan escape boundaries so that `\\u{...}` (an escaped backslash) is not
/// mistaken for a Unicode escape. Raw, byte, C, and character literals are excluded.
pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Vec<(Span, ReadingContent)> {
    if node.kind() != "string_literal" || node.has_error() { return Vec::new(); }
    let written = src.text(node);
    if !written.starts_with('"') || !written.ends_with('"') { return Vec::new(); }
    let bytes = written.as_bytes();
    let mut i = 1;
    let mut output = Vec::new();
    while i + 1 < bytes.len() {
        if bytes[i] != b'\\' { i += 1; continue; }
        if bytes.get(i + 1) == Some(&b'u') && bytes.get(i + 2) == Some(&b'{') {
            if let Some(relative_end) = written[i + 3..].find('}') {
                let close = i + 3 + relative_end;
                let digits = &written[i + 3..close];
                let hex: String = digits.chars().filter(|&ch| ch != '_').collect();
                if digits.chars().next().is_some_and(|ch| ch.is_ascii_hexdigit())
                    && digits.chars().all(|ch| ch.is_ascii_hexdigit() || ch == '_')
                    && (1..=6).contains(&hex.len())
                    && let Ok(codepoint) = u32::from_str_radix(&hex, 16)
                    && char::from_u32(codepoint).is_some()
                {
                    let span = src.index.span(src.file, node.start_byte() + i, node.start_byte() + close + 1);
                    output.push((span, ReadingContent::Detail {
                        label: "Unicode escape in a string".into(),
                        text: format!("`\\u{{{digits}}}` writes Unicode scalar value U+{codepoint:04X}: `\\u` starts the escape and the braces contain hexadecimal digits. In a normal Rust string it contributes that character encoded as UTF-8, not the literal characters of the escape. A scalar value excludes surrogate code points and values above U+10FFFF. A raw string does not interpret this escape."),
                        definition: None,
                        borrow: false,
                    }));
                    i = close + 1;
                    continue;
                }
            }
        }
        i += 2; // Consume an ordinary escape, notably `\\\\`, as one unit.
    }
    output
}
