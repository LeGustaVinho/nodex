use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn bounded_half_open_range_is_explained_alongside_existing_boundary_detail() {
    let source = "fn f() { let r = 0..10; let inclusive = 0..=10; let open = 0..; }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Create a range excluding its end")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[sites[0].span.start_byte as usize..sites[0].span.end_byte as usize], "..");
    assert!(matches!(&sites[0].content, ReadingContent::Detail { text, .. } if text.contains("0 through 9") && text.contains("does not create a list") && text.contains("one at a time")));
    assert!(flow.reading.iter().flatten().any(|site| site.span == sites[0].span && matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Range stops before the end")));
}
