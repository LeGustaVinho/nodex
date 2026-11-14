use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn half_open_range_pattern_is_explained_without_confusing_expressions_or_inclusive_patterns() {
    let text = "fn f(x: i32) -> bool { match x { 1..10 => true, _ => false } } fn other(x: i32) { let _r = 1..10; match x { 1..=10 => (), _ => () } }";
    let ir = lower_source("src/lib.rs", text);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Match a half-open range")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    assert_eq!(&text[sites[0].span.start_byte as usize..sites[0].span.end_byte as usize], "..");
    assert!(matches!(&sites[0].content, ReadingContent::Detail { text, .. }
        if text.contains("greater than or equal to 1") && text.contains("strictly less than 10")
        && text.contains("not 10") && text.contains("runtime local variables") && text.contains("pattern rather than creating a range value")));
    let other = ir.flows.iter().find(|flow| flow.name == "other").unwrap();
    assert!(!other.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Match a half-open range")));
}
