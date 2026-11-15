use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_and_assignment_for_integer_boolean_and_unknown_types() {
    let source = "fn f(mut x: u8, y: u8, mut flag: bool, condition: bool, mut custom: Value) { x &= y; flag &= condition; custom &= y; x |= y; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "AND and assign")).collect();
    assert_eq!(facts.len(), 3, "{:?}", flow.reading);
    for fact in facts {
        assert_eq!(&source[fact.span.start_byte as usize..fact.span.end_byte as usize], "&=");
        assert!(matches!(&fact.content, ReadingContent::Detail { text, .. } if text.contains("integer operands") && text.contains("booleans") && text.contains("without short-circuiting") && text.contains("BitAndAssign") && text.contains("types known before monomorphization") && text.contains("right side is evaluated first") && text.contains("produces `()`") && text.contains("does not prove")));
    }
}
