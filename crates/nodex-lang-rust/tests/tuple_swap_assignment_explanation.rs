use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn tuple_swap_explains_old_values_order_and_copy_or_move_uncertainty() {
    let source = "fn swap() { (a, b) = (b, a); } fn not_swap() { (a, b) = (a, b); } fn same() { (a, a) = (a, a); } fn calls() { (a, b) = (get_b(), a); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "swap").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Swap two values")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], "(a, b) = (b, a)");
    let ReadingContent::Detail { text, .. } = &facts[0].content else { unreachable!() };
    for phrase in ["a holds the old value of b", "b holds the old value of a", "entire right-hand tuple first", "left to right", "copied or moved", "does not resolve"] { assert!(text.contains(phrase), "{text}"); }
    for name in ["not_swap", "same", "calls"] {
        let flow = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        assert!(!flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Swap two values")), "{name}");
    }
}
