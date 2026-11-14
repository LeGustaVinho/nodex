use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn at_pattern_explains_whole_binding_and_nested_check() {
    let source = "fn f(v: i32) -> i32 { match v { name @ 1..=5 => name, _ => 0 } } fn ordinary(v: i32) { match v { 1..=5 => (), _ => () } }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let site = flow.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Bind the value matched by a pattern")).unwrap_or_else(|| panic!("{:?}", flow.reading));
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "@");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("entire matched value") && text.contains("1..=5") && text.contains("borrows, copies or moves") && text.contains("successful match")));
    let other = file.flows.iter().find(|flow| flow.name == "ordinary").unwrap();
    assert!(!other.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Bind the value matched by a pattern")));
}
