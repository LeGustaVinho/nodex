use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explicit_closure_result_requires_a_block() {
    let source = "fn f() { let a = |x: i32| -> i32 { x + 1 }; let b = |x: i32| x + 1; let c = |x: i32| { x + 1 }; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit closure result requires a block")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[details[0].span.start_byte as usize..details[0].span.end_byte as usize], "->");
    assert!(matches!(&details[0].content, ReadingContent::Detail { text, .. } if text.contains("must use a brace-delimited block") && text.contains("even when the body contains only one expression") && text.contains("without a semicolon") && text.contains("result type is omitted")));
}
