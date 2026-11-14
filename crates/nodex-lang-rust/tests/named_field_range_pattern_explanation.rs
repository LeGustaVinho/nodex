use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_named_field_inclusive_range_and_omitted_fields_in_flow() {
    let source = "struct Point { x: i32, y: i32 } fn f(p: Point) -> bool { match p { Point { x: 1..=5, .. } => true, _ => false } } fn other(p: Point) -> bool { match p { Point { x: 1..5, .. } => true, _ => false } }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let site = flow.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Check a named field and ignore the rest")).unwrap_or_else(|| panic!("{:?}", flow.reading));
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "..");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("Point") && text.contains("x field") && text.contains("1..=5") && text.contains("both written endpoints") && text.contains("not individually listed or bound")));
    let other = file.flows.iter().find(|flow| flow.name == "other").unwrap();
    assert!(!other.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Check a named field and ignore the rest")));
}
