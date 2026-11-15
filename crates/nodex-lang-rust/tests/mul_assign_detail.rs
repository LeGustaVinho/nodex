use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_multiply_assignment_on_primitives_and_unknown_types() {
    let source = "fn f(mut x: i32, factor: i32, mut custom: Value) { x *= factor; custom *= factor; x += factor; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Multiply and assign")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "*=");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("MulAssign") && text.contains("types known before monomorphization") && text.contains("right side is evaluated first") && text.contains("produces `()`") && text.contains("does not prove")));
    }
}
