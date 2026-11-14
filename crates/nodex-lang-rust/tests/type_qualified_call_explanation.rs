use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn bracketed_type_call_is_explained_without_confusing_trait_qualification() {
    let source = "fn f<T>() { <T>::method(); <T as Trait>::method(); Type::method(); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Call through a type")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "<T>");
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. } if text.contains("No trait is named") && text.contains("compiler resolves")));
}
