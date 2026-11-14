use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn open_end_range_explains_inclusive_start_and_slicing_without_claiming_infinite_iteration() {
    let source = "fn f(data: &[u8]) { let range = 10..; let tail = &data[10..]; let before = ..10; let bounded = 10..=12; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Range from a starting value")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "10");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("included starting bound") && text.contains("no written upper bound") && text.contains("does not itself iterate") && text.contains("bounds checks")));
    }
    assert!(flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Range has no end bound")).count() >= 2);
}
