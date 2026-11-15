use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn or_assignment_explains_integer_boolean_and_custom_behavior() {
    let source = "fn f(mut place: u8, value: u8, mut flag: bool) { place |= value; flag |= true; place &= value; let _ = true || flag; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "OR and assign")).collect();
    assert_eq!(items.len(), 2, "{:?}", flow.reading);
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "|=");
        assert!(matches!(&item.content, ReadingContent::Detail { text, .. } if text.contains("For integers") && text.contains("For booleans") && text.contains("not short-circuiting") && text.contains("BitOrAssign") && text.contains("does not establish")));
    }
}
