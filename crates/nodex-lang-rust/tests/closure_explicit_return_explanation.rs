use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn closure_arrow_explains_input_return_type_and_when_body_runs() {
    let source = "fn f() { let explicit = |x: i32| -> i32 { x + 1 }; let inferred = |x: i32| x + 1; }";
    let ir = lower_source("src/lib.rs", source);
    let items: Vec<_> = ir.flows.iter().flat_map(|flow| flow.reading.iter().flatten()).filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Closure with an explicit result type")).collect();
    assert!(!items.is_empty(), "{:#?}", ir.flows);
    for site in items {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "->");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("|x: i32|") && text.contains("produce i32") && text.contains("when the closure is called") && text.contains("without a semicolon")));
    }
}
