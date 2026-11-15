//! Explain a trait method returning an opaque iterator with a specified item type.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return None; }
    let mut parent = node.parent();
    let in_trait = loop {
        let Some(current) = parent else { break false; };
        if current.kind() == "trait_item" { break true; }
        if matches!(current.kind(), "impl_item" | "function_item" | "function_signature_item") { break false; }
        parent = current.parent();
    };
    if !in_trait { return None; }
    let result = node.child_by_field_name("return_type")?;
    let compact: String = src.text(result).chars().filter(|ch| !ch.is_whitespace()).collect();
    if result.kind() != "abstract_type" || compact != "implIterator<Item=u8>" { return None; }
    Some(ContractItem {
        group: "Trait method opaque iterator".into(),
        text: "This trait method promises a returned value of some concrete type chosen by the implementation, without naming that type in the signature. Different trait implementations may choose different concrete return types. If Iterator resolves to Rust's standard Iterator trait, Item = u8 says its produced items are unsigned 8-bit integers; callers can use the promised iterator operations without knowing the concrete iterator type. The return-position impl Trait in a trait method corresponds to an anonymous associated type. Generic and lifetime capture follows Rust's applicable rules; the exact captures and whether the returned value borrows from self depend on the signature and compiler rules, including edition and any precise capture bounds. This syntax alone does not establish a particular borrow, heap allocation, or dynamic dispatch; Iterator is subject to name resolution.".into(),
        span: src.span(result), type_info: None,
    })
}
