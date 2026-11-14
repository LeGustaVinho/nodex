use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn mutable_borrow_expression_is_visible_without_confusing_raw_or_shared_borrows() {
    let source = "fn f(mut x: i32) { let p = &mut x; *p += 1; let q = &x; let r = &raw mut x; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Borrow with exclusive mutable access"
    )).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    let detail = details[0];
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "mut");
    assert!(matches!(&detail.content, ReadingContent::Detail { text, .. }
        if text.contains("rather than transferring ownership") && text.contains("other accesses") && text.contains("reborrow")));
}
