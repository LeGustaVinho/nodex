use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn exclusive_range_to_explains_a_range_value_without_assuming_slicing() {
    let source = "fn f(end: i32) { let limits = ..end; let bounded = 0..end; let inclusive = ..=end; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Range with an exclusive upper bound")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    let detail = details[0];
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "end");
    assert!(matches!(&detail.content, ReadingContent::Detail { text, .. } if text.contains("no lower bound written") && text.contains("excluded") && text.contains("does not generally mean zero") && text.contains("not itself a sequence")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Range has no start bound")), "existing range detail should remain visible");
}
