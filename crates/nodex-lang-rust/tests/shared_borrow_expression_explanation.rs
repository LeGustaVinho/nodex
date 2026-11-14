use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn shared_borrow_expression_appears_without_confusing_mutable_and_raw() {
    let source = "fn f(mut value: i32) { let shared = &value; let changed = &mut value; let raw = &raw const value; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Create a shared borrow")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    let detail = details[0];
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "&");
    assert!(matches!(&detail.content, ReadingContent::Detail { text, .. }
        if text.contains("owner keeps") && text.contains("cannot outlive") && text.contains("interior mutability")));
}
