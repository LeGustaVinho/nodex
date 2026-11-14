use nodex_lang_rust::lower_source;
use nodex_ir::reading::ReadingContent;

#[test]
fn string_is_explained_conditionally_in_signatures_and_local_annotations() {
    let source = "type Alias = Vec<String>; fn f(s: String) -> String { let local: String = s; local } fn other(x: custom::String) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["Alias", "f"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let items = symbol.rust_contract.as_ref().unwrap();
        assert!(items.iter().any(|i| i.group == "Text type" && i.text.contains("If it resolves") && i.text.contains("UTF-8")), "{name}: {items:?}");
    }
    let other = ir.symbols.iter().find(|s| s.name == "other").unwrap();
    assert!(!other.rust_contract.as_ref().is_some_and(|items| items.iter().any(|i| i.group == "Text type")));
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Owned text type" && text.contains("If it resolves"))));
}
