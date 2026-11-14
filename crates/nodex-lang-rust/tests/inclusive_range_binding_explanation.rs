use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn range_binding_explains_both_inclusive_endpoints_and_success_scope() {
    let source = "fn f(value: i32) -> i32 { match value { n @ 1..=5 => n, _ => 0 } } fn g(value: i32) -> i32 { match value { n @ 1..5 => n, _ => 0 } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let item = flow.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Bind a value in an inclusive range")).expect("range-binding explanation on flow");
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "@");
    let ReadingContent::Detail { text, .. } = &item.content else { unreachable!() };
    assert!(text.contains("between 1 and 5, including both endpoints"));
    assert!(text.contains("binds the matched value to n"));
    assert!(text.contains("does not match"));
    let other = ir.flows.iter().find(|f| f.name == "g").unwrap();
    assert!(!other.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Bind a value in an inclusive range")));
}
