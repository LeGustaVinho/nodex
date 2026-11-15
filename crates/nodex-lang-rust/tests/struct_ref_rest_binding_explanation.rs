use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_shared_field_binding_and_rest_together() {
    let source = "struct Point { x: String, y: i32 } fn f(p: Point) { let Point { x: ref a, .. } = p; let Point { x: b, .. } = p; }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Borrow a field and omit the rest")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "ref a");
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. } if text.contains("x: ref a") && text.contains("shared reference") && text.contains("other fields") && text.contains("unbound") && text.contains("not constructing")));
}
