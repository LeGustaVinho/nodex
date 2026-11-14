use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_some_pattern_on_success_without_assuming_standard_option() {
    let source = "fn f(a: Option<String>) { if let Some(x) = a { use_value(x); } } fn g(a: Option<i32>) { if let None = a {} }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let item = flow.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Match a Some-shaped value")).expect("pattern explanation on flow");
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "Some");
    let ReadingContent::Detail { text, .. } = &item.content else { unreachable!() };
    assert!(text.contains("only if it matches"));
    assert!(text.contains("If Some resolves"));
    assert!(text.contains("moves, copies, or borrows"));
    let other = ir.flows.iter().find(|flow| flow.name == "g").unwrap();
    assert!(!other.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Match a Some-shaped value")));
}
