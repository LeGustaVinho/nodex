use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn addition_assignment_is_explained_without_assuming_operand_types() {
    let source = "fn f(mut place: i32, value: i32) { place += value; place -= value; let _ = place + value; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Add and assign")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "+=");
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. } if text.contains("place += value") && text.contains("unit value") && text.contains("overflow") && text.contains("AddAssign") && text.contains("mutable place") && text.contains("does not establish")));
}
