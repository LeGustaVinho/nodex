//! Source-linked explanation for a module declared in a separate file.

use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, item: Node<'_>) -> Option<ContractItem> {
    if item.kind() != "mod_item" || item.child_by_field_name("body").is_some() {
        return None;
    }
    let name = item.child_by_field_name("name")?;
    let name_text = src.norm(name);
    Some(ContractItem {
        group: "Module declaration".into(),
        text: format!("Declare the {name_text} module here; its contents come from another source file. Rust normally looks for {name_text}.rs or {name_text}/mod.rs relative to this module's directory. A #[path = ...] attribute can choose a different file. The semicolon ends the declaration; it does not mean the module is empty."),
        span: src.span(item),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn external_module_has_a_visible_source_linked_contract() {
        let ir = crate::lower_source("src/lib.rs", "mod utils; mod inline {} ");
        let symbol = ir.symbols.iter().find(|s| s.id == "ns:crate::utils").expect("external declaration symbol");
        let explanation = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Module declaration").unwrap();
        assert!(explanation.text.contains("utils.rs"));
        assert!(explanation.text.contains("#[path = ...]"));
        assert_eq!(explanation.span.start_byte, 0);
        let inline = ir.symbols.iter().find(|s| s.id == "ns:crate::inline").unwrap();
        assert!(!inline.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Module declaration")));
    }
}
