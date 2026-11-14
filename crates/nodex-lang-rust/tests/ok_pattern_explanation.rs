use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_ok_pattern_and_does_not_assume_standard_result() {
    let text = "fn f(result: Result<i32, ()>) { match result { Ok(value) => consume(value), Err(_) => fail() } let Ok(other) = result else { return; }; }";
    let ir = lower_source("src/lib.rs", text);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Match an Ok-shaped value")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    assert!(facts.iter().all(|site| matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("If Ok resolves to the standard Result") && text.contains("does not prove") && text.contains("moved, copied, or borrowed"))));
}
