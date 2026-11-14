use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_fully_qualified_associated_types_in_contract_and_local_flow() {
    let source = "trait Trait { type Item; } fn use_item<T: Trait>(x: <T as Trait>::Item) -> <T as Trait>::Item { let y: <T as Trait>::Item = x; y } fn ordinary<T>(x: T::Item) {}";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|s| s.name == "use_item").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|i| i.group == "Qualified associated type").collect();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|i| i.text.contains("implementation of trait Trait for type T") && i.text.contains("not a value")));
    let flow = ir.flows.iter().find(|f| f.name == "use_item").unwrap();
    assert!(flow.reading.iter().flatten().any(|r| matches!(&r.content, ReadingContent::Detail { label, text, .. } if label == "Qualified associated type" && text.contains("disambiguate"))), "{:?}", flow.reading);
    let ordinary = ir.symbols.iter().find(|s| s.name == "ordinary").unwrap();
    assert!(!ordinary.rust_contract.as_ref().into_iter().flatten().any(|i| i.group == "Qualified associated type"));
}
