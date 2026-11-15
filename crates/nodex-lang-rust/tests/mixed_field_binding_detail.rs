use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_ref_and_mut_shorthand_in_one_struct_pattern() {
    let source = "struct Point { x: String, y: i32 } fn f(point: Point) { let Point { ref x, mut y } = point; let Point { x, y } = point; }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Mixed field binding modes")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "Point { ref x, mut y }");
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. } if text.contains("ref x") && text.contains("shared reference") && text.contains("mut y") && text.contains("may be reassigned") && text.contains("copied or moved") && text.contains("name resolution")));
}
