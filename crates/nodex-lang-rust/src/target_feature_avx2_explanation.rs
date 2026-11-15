//! Explain the `target_feature(enable = "avx2")` outer function attribute.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_item" { return Vec::new(); }
    let mut previous = node.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        if attribute.kind() == "attribute_item"
            && let Some((prefix, suffix)) = src.text(attribute).split_once("\"avx2\"")
        {
            // Keep the quoted feature name exact; whitespace inside it is significant.
            let prefix: String = prefix.chars().filter(|ch| !ch.is_whitespace()).collect();
            let suffix: String = suffix.chars().filter(|ch| !ch.is_whitespace()).collect();
            if prefix == "#[target_feature(enable=" && suffix == ")]" {
                result.push(ContractItem {
                    group: "AVX2 target feature".into(),
                    text: "`#[target_feature(enable = \"avx2\")]` enables the AVX2 target feature when compiling this function for a supported target. The compiler may generate AVX2 instructions in its body. The attribute does not add AVX2 support to the CPU or perform a runtime check. Calling the function on a CPU that lacks the required feature is undefined behavior; callers must establish support, for example through suitable target configuration or runtime detection. When the feature is not known to be enabled for the caller, Rust requires an unsafe call. This annotation alone does not prove that the target, CPU, or call is valid.".into(),
                    span: src.span(attribute), type_info: None,
                });
            }
        }
        previous = attribute.prev_named_sibling();
    }
    result
}
