//! Explain a test's expected panic message without claiming the test will pass.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

/// Recognize only the explicit outer `#[should_panic(expected = "...")]`
/// form; other meta forms and conditional attributes need separate handling.
fn expected_string<'t>(src: Src<'_>, item: Node<'t>) -> Option<Node<'t>> {
    if item.kind() != "attribute_item" || item.has_error() { return None; }
    let attr = named_children(item).into_iter().find(|part| part.kind() == "attribute")?;
    let name = named_children(attr).into_iter().find(|part| part.kind() == "identifier")?;
    if src.text(name) != "should_panic" { return None; }
    let args = attr.child_by_field_name("arguments")?;
    if args.kind() != "token_tree" || args.has_error() { return None; }
    let tokens: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
    let (key, equal, value) = match tokens.as_slice() {
        [open, key, equal, value, close] if open.kind() == "(" && close.kind() == ")" => (*key, *equal, *value),
        [open, key, equal, value, comma, close] if open.kind() == "(" && comma.kind() == "," && close.kind() == ")" => (*key, *equal, *value),
        _ => return None,
    };
    if key.kind() != "identifier" || src.text(key) != "expected" || equal.kind() != "="
        || !matches!(value.kind(), "string_literal" | "raw_string_literal") || value.has_error() { return None; }
    Some(value)
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let mut result = Vec::new();
    let mut previous = node.prev_named_sibling();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        if let Some(value) = expected_string(src, attribute) {
            result.push(ContractItem {
                group: "Expected panic message".into(),
                text: format!("When this function is run as a test by Rust's test harness, #[should_panic(expected = {})] requires it to panic and for the panic message to contain the specified string; the string need not be the entire message. The expected text is the Rust string literal as written here, with its escapes interpreted according to Rust's string rules. If the test returns normally, or the observed panic message does not contain the expected text, the test fails. This attribute does not itself trigger a panic or register the function as a test; #[test] or an applicable test framework supplies that role.", src.text(value)),
                span: src.span(value), type_info: None,
            });
        }
        previous = attribute.prev_named_sibling();
    }
    result
}
