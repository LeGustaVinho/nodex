use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_invalid_tuple_index_spellings_without_claiming_type() {
    let source = "fn f(tuple: (i32, i32)) { let a = tuple.01; let b = tuple.0usize; let c = tuple.0; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Invalid tuple-field spelling")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for fact in &facts { assert!(matches!(&fact.content, ReadingContent::Detail { text, .. } if text.contains("invalid") && text.contains("leading zeros") && text.contains("type suffix") && text.contains("require compiler checking") && text.contains("not every"))); }
    assert!(facts.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("leading zero") && text.contains("`.1`"))));
    assert!(facts.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("`usize` suffix") && text.contains("`.0`"))));
}
