//! Explain `#[proc_macro]` applied to a function-like procedural macro.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_item" { return None; }
    let mut previous = node.prev_sibling();
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" => {
                let compact: String = src.text(part).chars().filter(|character| !character.is_whitespace()).collect();
                if compact == "#[proc_macro]" {
                    let name = node.child_by_field_name("name").map(|name| src.norm(name)).unwrap_or_else(|| "macro_name".into());
                    return Some(ContractItem {
                        group: "Function-like procedural macro".into(),
                        text: format!("`#[proc_macro]` marks this function as the definition of a function-like procedural macro, invoked from another crate as `{name}!(...)`. During compilation, the compiler passes the tokens inside the invocation delimiters as a `proc_macro::TokenStream`; the function returns a token stream that replaces the invocation before the resulting code is checked. These are syntax tokens, not ordinary runtime arguments. For this attribute to be valid, the function must be public, have the Rust ABI and the required `fn(TokenStream) -> TokenStream` signature, and be at the root of a `proc-macro` crate. The written attribute alone does not prove that the crate configuration or function signature meets those requirements."),
                        span: src.span(part), type_info: None,
                    });
                }
            }
            "line_comment" if !src.text(part).starts_with("//!") => {},
            "block_comment" if !src.text(part).starts_with("/*!") => {},
            _ => break,
        }
        previous = part.prev_sibling();
    }
    None
}
