//! Explain a Unicode scalar escape inside a Rust character literal.
use nodex_ir::{Span, reading::ReadingContent};
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

/// Bridge tree-sitter's rejection of underscores in valid character escapes.
/// Only the parser sees this same-length spelling; all spans and displayed text
/// continue to refer to the user's original source. `parse` keeps a rewrite only
/// when it lowers the syntax-error count.
pub(crate) fn repair_unicode_char_underscores(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut rewritten = bytes.to_vec();
    let mut changed = false;
    let mut i = 0;
    while i + 4 < bytes.len() {
        if !bytes[i..].starts_with(b"'\\u{") || (i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_')) {
            i += 1;
            continue;
        }
        let start = i + 4;
        let mut end = start;
        while end < bytes.len() && (bytes[end].is_ascii_hexdigit() || bytes[end] == b'_') { end += 1; }
        if end == start || !bytes[start].is_ascii_hexdigit() || bytes.get(end..end + 2) != Some(b"}'") {
            i += 1;
            continue;
        }
        let digits: String = bytes[start..end].iter().filter(|byte| **byte != b'_').map(|byte| *byte as char).collect();
        let valid = digits.len() <= 6 && u32::from_str_radix(&digits, 16).ok().and_then(char::from_u32).is_some();
        if valid {
            for position in start..end {
                if bytes[position] == b'_' { rewritten[position] = b'0'; changed = true; }
            }
        }
        i = end + 2;
    }
    changed.then(|| String::from_utf8(rewritten).expect("ASCII replacements preserve UTF-8"))
}

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    if node.kind() != "char_literal" || node.has_error() { return None; }
    let written = src.text(node);
    let digits = written.strip_prefix("'\\u{")?.strip_suffix("}'")?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_hexdigit() || c == '_') { return None; }
    let compact: String = digits.chars().filter(|c| *c != '_').collect();
    if compact.is_empty() || compact.len() > 6 { return None; }
    let scalar = u32::from_str_radix(&compact, 16).ok()?;
    char::from_u32(scalar)?;
    // Highlight the escape, not its surrounding quotes or the entire expression.
    let span = src.index.span(src.file, node.start_byte() + 1, node.end_byte() - 1);
    Some((span, ReadingContent::Detail {
        label: "Unicode character escape".into(),
        text: format!("This character literal uses the `\\u{{...}}` escape: the digits inside the braces give a hexadecimal Unicode scalar value (here U+{scalar:04X}). Rust interprets the escape as that one character, not the literal backslash and letters. A Rust `char` is one Unicode scalar value represented in four bytes; it is not necessarily one visible character (grapheme) or one UTF-16 code unit. The value must be a valid Unicode scalar, so surrogate code points and values above U+10FFFF are not valid. Underscores can separate hexadecimal digits without changing their value."),
        definition: None, borrow: false,
    }))
}

#[cfg(test)]
mod tests {
    use super::repair_unicode_char_underscores;

    #[test]
    fn repair_preserves_offsets_and_ignores_invalid_or_prefixed_literals() {
        let source = r"let a = '\u{1f_600}'; let b = '\u{10_f_f_ff}';";
        let repaired = repair_unicode_char_underscores(source).unwrap();
        assert_eq!(repaired.len(), source.len());
        assert_eq!(repaired, r"let a = '\u{1f0600}'; let b = '\u{100f0f0ff}';");
        assert!(repair_unicode_char_underscores(r"let a = '\u{d_800}'; let b = b'\u{1_f}';").is_none());
    }
}
