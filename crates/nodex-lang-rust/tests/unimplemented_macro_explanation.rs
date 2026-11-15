use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn empty_unimplemented_macro_is_explained_without_assuming_resolution() {
    let source = "fn f() { unimplemented!(); unimplemented!(\"message\"); other!(); }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Placeholder that panics when reached")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "unimplemented");
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. } if text.contains("If this resolves") && text.contains("panics") && text.contains("unwind or abort") && text.contains("shadowed")));
}
