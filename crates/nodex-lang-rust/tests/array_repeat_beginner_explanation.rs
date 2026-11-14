use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn array_repetition_explains_zero_and_const_exceptions_alongside_existing_detail() {
    let source = "const EMPTY: Vec<i32> = Vec::new(); fn f() { let a = [0; 100]; let b = [String::new(); 0]; let c = [const { Vec::<i32>::new() }; 2]; let d = [EMPTY; 0]; let list = [1, 2]; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Understand array repetition")).collect();
    assert_eq!(details.len(), 4, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "[");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("compile-time `usize`") && text.contains("evaluates it once even when the length is zero") && text.contains("path to a constant item") && text.contains("not evaluated when the length is zero") && text.contains("does not call `Clone`")));
    }
    let existing = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Build an array by repetition")).count();
    assert_eq!(existing, 4, "existing specific repeat details should remain visible");
}
