use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_raw_const_address_without_confusing_borrows_or_raw_mut() {
    let source = "fn f(mut value: i32) { let p = &raw const value; let q = &raw mut value; let borrowed = &value; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Create a raw const pointer")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    let detail = details[0];
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "raw");
    assert!(matches!(&detail.content, ReadingContent::Detail { text, .. }
        if text.contains("without first creating a Rust reference") && text.contains("Dereferencing the pointer requires unsafe") && text.contains("does not make the underlying storage immutable")));
}
