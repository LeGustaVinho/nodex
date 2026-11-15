//! Explain a two-digit hexadecimal escape in a character literal.
use nodex_ir::{Span, reading::ReadingContent};
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    if node.kind() != "char_literal" || node.has_error() { return None; }
    let text = src.text(node);
    let bytes = text.as_bytes();
    if bytes.len() != 6 || bytes[0] != b'\'' || bytes[1] != b'\\' || bytes[2] != b'x' || bytes[5] != b'\'' { return None; }
    let hex = text.get(3..5)?;
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) { return None; }
    let value = u8::from_str_radix(hex, 16).ok()?;
    // A non-byte character literal only accepts ASCII values for `\xNN`.
    if value > 0x7f { return None; }
    let printable = if (0x20..=0x7e).contains(&value) {
        format!(" (the printable character `{}`)", char::from(value).escape_default())
    } else { String::new() };
    let span = src.index.span(src.file, node.start_byte() + 1, node.start_byte() + 5);
    Some((span, ReadingContent::Detail {
        label: "Hexadecimal character escape".into(),
        text: format!("In {}, `\\x{hex}` uses exactly two hexadecimal digits to specify the character U+{value:04X}{printable}. In an ordinary Rust `char` or string literal, this escape is limited to ASCII values 00 through 7F; higher Unicode scalar values can use `\\u{{...}}`. This literal is one `char` value, not a byte literal; byte literals use a `b` prefix and have different escape limits.", text),
        definition: None,
        borrow: false,
    }))
}
