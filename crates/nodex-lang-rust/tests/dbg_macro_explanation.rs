use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn dbg_single_expression_explains_output_evaluation_and_ownership_conditionally() {
    let source = "fn f(x: i32) { let result = dbg!(x + 1); other!(x); dbg!(x, 2); dbg!(); }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Debug a value with dbg!")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "dbg");
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. } if text.contains("If this invocation resolves") && text.contains("evaluated once") && text.contains("standard error") && text.contains("returns that same value") && text.contains("can move") && text.contains("not limited to debug builds")));
}
