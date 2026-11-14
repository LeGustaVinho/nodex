//! Explain generic inherent implementations on their associated items.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

/// A method or other associated item's parent is the body of the impl. Show the
/// implementation on the item itself because the impl block has no symbol.
pub(crate) fn explain(src: Src<'_>, member: Node<'_>) -> Option<ContractItem> {
    if !matches!(member.kind(), "function_item" | "function_signature_item" | "const_item" | "type_item") { return None; }
    let body = member.parent()?;
    if body.kind() != "declaration_list" { return None; }
    let implementation = body.parent()?;
    if implementation.kind() != "impl_item" || implementation.child_by_field_name("trait").is_some() { return None; }
    let parameters = implementation.child_by_field_name("type_parameters")?;
    let ty = implementation.child_by_field_name("type")?;
    let where_clause = nodex_lang_api::syntax::named_children(implementation).into_iter().find(|n| n.kind() == "where_clause");
    let restriction = where_clause.map(|clause| format!(" The written {} adds requirements that must hold.", src.norm(clause))).unwrap_or_default();
    Some(ContractItem {
        group: "Generic implementation".into(),
        text: format!("impl {} introduces parameters for this implementation, and {} is the type receiving the associated items in these braces. A type parameter such as T stands for a type chosen by each use, so the implementation can apply to a family of types. For example, T might be i32 or String when it satisfies the written bounds. The same parameter name connects its declaration after impl to its use in the type. These are inherent items belonging to the type, rather than an implementation of a named trait.{}", src.norm(parameters), src.norm(ty), restriction),
        span: src.header_span(implementation, implementation.child_by_field_name("body")),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::{kit::LineIndex, syntax::named_children};

    #[test]
    fn detects_generic_inherent_items_only() {
        let text = "struct Wrapper<T>(T); impl<T: Clone> Wrapper<T> where T: Send { fn f(&self) {} } impl Wrapper<i32> { fn concrete(&self) {} } impl<T> Trait for Wrapper<T> { fn trait_f(&self) {} }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "generic.rs", index: &index };
        let mut pending = vec![tree.root_node()];
        let mut found = Vec::new();
        while let Some(node) = pending.pop() {
            if let Some(item) = explain(src, node) { found.push(item.text); }
            pending.extend(named_children(node));
        }
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("Wrapper<T>"));
        assert!(found[0].contains("where T: Send"));
        assert!(found[0].contains("rather than an implementation of a named trait"));
    }
}
