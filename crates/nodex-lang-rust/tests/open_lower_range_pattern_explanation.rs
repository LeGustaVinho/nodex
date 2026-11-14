use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_open_upper_bound_as_a_pattern_not_a_range_value() {
    let source = "fn f(value: i32) -> i32 { match value { 10.. => 1, 1..=9 => 2, _ => 0 } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Match from a lower bound")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[details[0].span.start_byte as usize..details[0].span.end_byte as usize], "..");
    assert!(matches!(&details[0].content, ReadingContent::Detail { text, .. }
        if text.contains("equal to 10 or greater") && text.contains("omitted upper bound") && text.contains("does not create or iterate a range")));
}
