use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_full_range_index_conditionally_and_preserves_range_detail() {
    let source = "fn f(a: &[i32]) { let all = &a[..]; let partial = &a[1..]; let custom = object[..]; }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Index with the full range")).collect();
    assert_eq!(items.len(), 2, "{:?}", flow.reading);
    for site in items {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "..");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("If the operand is an array or slice") && text.contains("does not copy") && text.contains("Other operand types") && text.contains("resolved type")));
        assert!(flow.reading.iter().flatten().any(|other| other.span == site.span && matches!(&other.content, ReadingContent::Detail { label, .. } if label == "Range has no bounds")));
    }
}
