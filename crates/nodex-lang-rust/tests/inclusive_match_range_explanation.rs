use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn inclusive_match_pattern_explains_both_endpoints_without_confusing_range_expression() {
    let source = "fn check(n: i32) -> bool { let _r = 1..=10; match n { 1..=10 => true, _ => false } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "check").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Match an inclusive range")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    let item = items[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "..=");
    assert!(matches!(&item.content, ReadingContent::Detail { text, .. }
        if text.contains("at least 1") && text.contains("at most 10")
        && text.contains("includes both endpoints") && text.contains("not a loop")));
}
