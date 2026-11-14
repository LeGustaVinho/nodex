use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn inclusive_range_to_expression_is_distinct_from_pattern_and_assignment() {
    let source = "fn f(x: i32, xs: &[u8]) { let range = ..=10; let part = &xs[..=2]; match x { ..=10 => hit(), _ => miss() } let other = ..10; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Create a range through an inclusive end")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    assert!(facts.iter().all(|site| matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("no lower bound written") && text.contains("includes") && text.contains("does not enumerate") && text.contains("range pattern"))));
    assert!(facts.iter().all(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == "="));
}
