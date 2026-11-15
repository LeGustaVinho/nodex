use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_no_argument_todo_conditionally_on_macro_identity() {
    let source = "macro_rules! todo { () => { 1 } } fn f() { let x = todo!(); let y = todo!(\"later\"); let z = other!(); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Placeholder macro invocation")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[details[0].span.start_byte as usize..details[0].span.end_byte as usize], "!");
    assert!(matches!(&details[0].content, ReadingContent::Detail { text, .. } if text.contains("If `todo!` resolves") && text.contains("panics when execution reaches") && text.contains("does not by itself prevent compilation") && text.contains("different macro")));
}
