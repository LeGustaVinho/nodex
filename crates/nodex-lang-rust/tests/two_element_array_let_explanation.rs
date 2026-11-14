use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn ordinary_two_element_array_let_explains_positions_and_irrefutability() {
    let source = "fn f(array: [i32; 2], slice: &[i32]) { let [a, b] = array; if let [c, d] = slice { consume(c, d); } let [..] = array; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Bind two array positions")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    let site = facts[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "[");
    let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
    assert!(text.contains("a receives position 0"));
    assert!(text.contains("b receives position 1"));
    assert!(text.contains("irrefutable"));
    assert!(text.contains("fixed-size array of exactly two"));
    assert!(text.contains("slice of unknown length"));
    assert!(text.contains("name resolution"));
    assert!(flow.reading.iter().flatten().any(|site| matches!(site.content, ReadingContent::Pattern { .. })));
}
