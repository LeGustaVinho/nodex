use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn divide_assign_explains_in_place_operation_without_guessing_types() {
    let source = "fn f(mut x: i32, y: i32) { x /= y; x += y; let text = \"/=\"; }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Divide and assign to an existing place")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "/=");
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. } if text.contains("x /= y") && text.contains("DivAssign") && text.contains("user-defined") && text.contains("division by zero panics") && text.contains("does not establish") && text.contains("produces ()")));
}
