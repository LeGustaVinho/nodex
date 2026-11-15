use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn numeric_field_pattern_maps_positions_to_bindings() {
    let source = "fn f(pair: Pair) { let Pair { 0: a, 1: b } = pair; let Named { a, b } = named; let Pair(x, y) = pair; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Match numeric fields in braces")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    assert!(matches!(&facts[0].content, ReadingContent::Detail { text, .. } if text.contains("field 0 binds a") && text.contains("field 1 binds b") && text.contains("not array indexing") && text.contains("irrefutable") && text.contains("does not establish")));
    assert!(!flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Pattern { parts, .. } if parts.iter().any(|part| part.projection == "Pair.a" || part.projection == "Pair.b"))));
}
