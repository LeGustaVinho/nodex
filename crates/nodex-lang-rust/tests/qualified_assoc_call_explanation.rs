use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_call_through_qualified_associated_type() {
    let source = "fn f<T>() { <T as Trait>::Assoc::function(); <T as Trait>::method(); <T>::function(); let _ = <T as Trait>::Assoc::VALUE; }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Call through a trait-associated type")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[details[0].span.start_byte as usize..details[0].span.end_byte as usize], "<T as Trait>::Assoc::function");
    assert!(matches!(&details[0].content, ReadingContent::Detail { text, .. } if text.contains("trait Trait for type T") && text.contains("::Assoc") && text.contains("::function") && text.contains("compiler still resolves")));
}
