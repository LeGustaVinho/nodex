use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explicit_std_drop_explains_ownership_copy_and_references() {
    let source = "fn f(value: String, n: i32) { std::mem::drop(value); ::std::mem::drop(n); other::drop(n); drop(n); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Drop a value early")).collect();
    assert_eq!(items.len(), 2, "{:?}", flow.reading);
    for item in items {
        assert!((&source[item.span.start_byte as usize..item.span.end_byte as usize]).ends_with("std::mem::drop"));
        assert!(matches!(&item.content, ReadingContent::Detail { text, .. } if text.contains("If ") && text.contains("non-Copy") && text.contains("drops a copy") && text.contains("reference") && text.contains("does not establish name resolution")));
    }
}
