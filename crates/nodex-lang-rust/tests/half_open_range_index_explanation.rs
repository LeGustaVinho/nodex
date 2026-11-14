use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn range_index_is_explained_without_assuming_receiver_type() {
    let source = "fn f(array: &[i32]) { let part = &array[1..4]; let r = 1..4; let item = array[1]; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Index with a half-open range")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    let site = sites[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "[");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("stops before 4")
        && text.contains("If the receiver is a standard array or slice") && text.contains("invalid bounds panic")
        && text.contains("Other types can define indexing behavior")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Range stops before the end")));
}
