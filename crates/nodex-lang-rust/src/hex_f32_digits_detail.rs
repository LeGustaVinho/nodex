//! Distinguish hexadecimal digits `f32` from a floating-point suffix.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "integer_literal" || node.has_error() { return None; }
    let written = src.text(node);
    let digits = written.strip_prefix("0x")?;
    let digits: String = digits.chars().filter(|ch| *ch != '_').collect();
    if !digits.ends_with("f32") || !digits.chars().all(|ch| ch.is_ascii_hexdigit()) { return None; }
    let decimal = u128::from_str_radix(&digits, 16).ok()
        .map(|value| format!(" Its mathematical value is {value} in decimal."))
        .unwrap_or_default();
    Some((node, ReadingContent::Detail {
        label: "Hexadecimal digits, not an f32 suffix".into(),
        text: format!("{written} is a hexadecimal integer literal: 0x selects base 16, and underscores separate digits without changing the value. Here f, 3 and 2 are all hexadecimal digits, so the final f32 is part of the number rather than a floating-point type suffix.{decimal} Its integer type is determined by the surrounding context or integer fallback rules; the spelling alone does not make it a floating-point value. Rust still checks that the value fits the selected integer type."),
        definition: None,
        borrow: false,
    }))
}
