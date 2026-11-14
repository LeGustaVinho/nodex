use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_qualified_trait_call_without_confusing_other_paths() {
    let source = "fn f<T>(x: &T) { <T as Trait>::method(x); <T>::method(); Trait::method(x); let _ = <T as Trait>::ITEM; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Call an item through a trait implementation")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    assert!(matches!(&facts[0].content, ReadingContent::Detail { text, .. }
        if text.contains("trait Trait for type T") && text.contains("receiver is supplied explicitly") && text.contains("compiler must still resolve")));
}
