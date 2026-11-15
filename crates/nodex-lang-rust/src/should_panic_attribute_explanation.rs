//! Explain the bare `#[should_panic]` attribute on a function.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_item" { return Vec::new(); }
    let mut previous = node.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        if attribute.kind() == "attribute_item" {
            let written: String = src.text(attribute).chars().filter(|ch| !ch.is_whitespace()).collect();
            if written == "#[should_panic]" {
                result.push(ContractItem {
                    group: "Expected panic".into(),
                    text: "`#[should_panic]` marks an expected panic for a test. When this function is run as a test by the Rust test harness, the test passes if it panics and fails if it returns normally. The attribute describes the expected outcome; it does not itself cause a panic or prove the body will panic. This bare form does not require a particular panic message. Test discovery and execution still depend on the surrounding test configuration.".into(),
                    span: src.span(attribute), type_info: None,
                });
            }
        }
        previous = attribute.prev_named_sibling();
    }
    result
}
