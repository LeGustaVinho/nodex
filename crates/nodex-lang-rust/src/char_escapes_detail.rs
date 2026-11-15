//! Explain simple escapes in ordinary Rust character literals.
use nodex_ir::{Span, reading::ReadingContent};
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    if node.kind() != "char_literal" || node.has_error() { return None; }
    let literal = src.text(node);
    let (name, code, detail) = match literal {
        r"'\n'" => ("line feed (newline)", "U+000A", "It represents a newline control character."),
        r"'\t'" => ("horizontal tab", "U+0009", "It represents a tab control character."),
        r"'\0'" => ("NUL", "U+0000", "It is the zero-valued character, not the digit zero or an automatic string terminator."),
        r"'\\'" => ("backslash", "U+005C", "The doubled backslash writes one backslash character."),
        r"'\''" => ("single quote", "U+0027", "Escaping the quote keeps it from ending the character literal."),
        r#"'\"'"# => ("double quote", "U+0022", "The escape writes one double quote character; a double quote need not be escaped inside a character literal."),
        r"'\r'" => ("carriage return", "U+000D", "It represents a carriage-return control character, distinct from line feed."),
        _ => return None,
    };
    let span = src.index.span(src.file, node.start_byte() + 1, node.end_byte() - 1);
    Some((span, ReadingContent::Detail {
        label: "Simple character escape".into(),
        text: format!("{literal} is one Rust `char`: the escape between the single-quote delimiters represents {name}, Unicode scalar {code}. {detail} The backslash and following symbol are source notation for one value, not two characters. This is an ordinary `char` literal, not a byte literal or a string."),
        definition: None, borrow: false,
    }))
}
