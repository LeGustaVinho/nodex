use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_named_continue_from_nested_loop_without_claiming_loop_exits() {
    let source = "fn f() { 'outer: while ready() { loop { continue 'outer; } } } fn g() { loop { continue; } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let item = flow.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Continue a named loop")).expect("named continue detail on the flow");
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "'outer");
    let ReadingContent::Detail { text, .. } = &item.content else { unreachable!() };
    assert!(text.contains("skips the rest"));
    assert!(text.contains("checks its condition again"));
    assert!(text.contains("does not exit the named loop"));
    let plain = ir.flows.iter().find(|flow| flow.name == "g").unwrap();
    assert!(!plain.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Continue a named loop")));
}
