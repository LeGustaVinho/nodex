use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_inclusive_prefix_index_without_assuming_receiver_type() {
    let source = "fn f(array: &[i32], custom: Custom) { let a = &array[..=4]; let b = &custom[..=4]; let c = &array[..4]; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Index through an inclusive end")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "[");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("first five") && text.contains("panics") && text.contains("may implement indexing differently") && text.contains("does not prove")));
    }
}
