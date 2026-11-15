use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn subtraction_assignment_explains_types_trait_and_order_without_guessing() {
    let source = "fn f() { x -= value; values[index()] -= compute(); x += 1; let text = \"-=\"; m!(x -= y); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Subtract and assign")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for fact in facts {
        assert_eq!(&source[fact.span.start_byte as usize..fact.span.end_byte as usize], "-=");
        let ReadingContent::Detail { text, .. } = &fact.content else { unreachable!() };
        for phrase in ["mutable place", "SubAssign", "operand types", "produces `()`", "right side first", "does not establish", "overflow behavior"] { assert!(text.contains(phrase), "{text}"); }
    }
}
