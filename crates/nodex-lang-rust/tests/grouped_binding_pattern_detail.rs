use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn grouped_let_binds_whole_value_not_a_single_element_tuple() {
    let source = "fn f() { let (x) = value; let ((nested)) = other; let (single,) = tuple; let direct = value; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Grouped binding pattern")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for (pattern, binding) in [("(x)", "x"), ("((nested))", "nested")] {
        let site = facts.iter().find(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == pattern).unwrap();
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        for phrase in ["without a comma", "entire value", "one-element tuple", "copy or move", binding] { assert!(text.contains(phrase), "{text}"); }
    }
}
