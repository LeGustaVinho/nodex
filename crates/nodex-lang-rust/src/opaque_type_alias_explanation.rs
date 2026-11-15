//! Explain experimental opaque type aliases using `impl Trait`.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "type_item" || node.has_error() { return None; }
    let name = node.child_by_field_name("name")?;
    let ty = node.child_by_field_name("type")?;
    if ty.kind() != "abstract_type" { return None; }
    Some(ContractItem {
        group: "Opaque type alias".into(),
        text: format!("`type {} = {}` declares an opaque type alias: it names a concrete type hidden behind the listed trait bounds. Code using the alias can rely on those bounds without the alias exposing the concrete type. The hidden type is constrained by the alias's defining uses; this is not a choice of a different arbitrary type at each use, and does not by itself require heap allocation or dynamic dispatch. `impl Trait` in a type alias is experimental and requires the unstable `type_alias_impl_trait` feature and a supporting compiler configuration. The spelling alone neither identifies the hidden type nor proves that the feature is enabled or the bounds resolve as expected.", src.norm(name), src.norm(ty)),
        span: src.span(ty), type_info: None,
    })
}
