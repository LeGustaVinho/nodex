use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn distinguishes_destructuring_assignment_from_constructor_call() {
    let source = "fn f() { Pair(a, b) = pair; module::Pair(x, y) = other; Pair(a, b); (a, b) = pair; let Pair(c, d) = pair; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Assign tuple-struct fields")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for fact in facts {
        assert!(matches!(&fact.content, ReadingContent::Detail { text, .. } if text.contains("not a function call") && text.contains("first and second fields") && text.contains("existing places") && text.contains("does not declare") && text.contains("irrefutability") && text.contains("does not prove")));
    }
}
