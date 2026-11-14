use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_or_pattern_on_match_without_guessing_binding_values() {
    let source = "enum E { A(i32), B(i32), C } fn f(v: E) -> i32 { match v { E::A(x) | E::B(x) if x > 0 => x, _ => 0 } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Alternative patterns"
    )).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[details[0].span.start_byte as usize..details[0].span.end_byte as usize], "|");
    assert!(matches!(&details[0].content, ReadingContent::Detail { text, .. }
        if text.contains("same names") && text.contains("same binding modes") && text.contains("failed guard")));
}
