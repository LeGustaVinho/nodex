use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn assignment_maps_fields_to_existing_places_and_keeps_existing_steps() {
    let source = "struct Point { x: i32, y: i32 } fn f(p: Point) { let (mut a, mut b) = (0, 0); Point { x: a, y: b } = p; let q = Point { x: 1, y: 2 }; (a, b) = (1, 2); }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Assign named fields to existing places")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    let item = items[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "=");
    assert!(matches!(&item.content, ReadingContent::Detail { text, .. } if text.contains("field x goes to a") && text.contains("field y goes to b") && text.contains("evaluate p once") && text.contains("existing places") && text.contains("left to right") && text.contains("moved or copied")));
    assert!(flow.reading.iter().flatten().any(|site| site.node == item.node && matches!(&site.content, ReadingContent::Steps { label, .. } if label == "Assign parts to existing destinations")));
}
