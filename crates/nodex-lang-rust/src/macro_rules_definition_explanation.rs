//! Describe a declarative macro definition in the symbol inspector.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "macro_definition" || node.has_error() { return None; }
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "Declarative macro definition".into(),
        text: format!("`macro_rules! {}` defines a declarative macro named {}. Its braces contain rules: each rule has a matcher describing input tokens and a template describing the tokens to produce. At an invocation such as `{}!(...)`, the compiler tries the matchers in written order and expands the first matching rule; it then checks the resulting Rust syntax in its context. If no rule matches, expansion fails. A macro invocation supplies tokens, not ordinary function arguments evaluated before a call. The definition itself does not execute the code a later invocation may generate. The name and rule details must be understood in their scope and syntax context.", src.norm(name), src.norm(name), src.norm(name)),
        span: src.span(name),
        type_info: None,
    })
}
