use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_trailing_dot_float_without_confusing_ranges_or_other_literals() {
    let source = "fn f() { let a = 2.; let b = 2.0; let c = 2..4; let d: f32 = 3.; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Floating-point literal with trailing dot")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], ".");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("no digits follow") && text.contains("f32") && text.contains("default is `f64`") && text.contains("range operator")));
    }
}
