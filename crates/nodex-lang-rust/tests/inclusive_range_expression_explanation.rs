use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn bounded_inclusive_range_explains_both_endpoints_in_flow() {
    let source = "fn f() { let a = 0..=10; let b = 0..10; consume(a,b); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let site = flow.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Range includes the end")).expect("inclusive range explanation on flow");
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "..=");
    let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
    assert!(text.contains("0 as its start bound"));
    assert!(text.contains("10 as its end bound"));
    assert!(text.contains("Both bounds are included"));
    assert!(text.contains("element type and permitted operations depend on context"));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Range stops before the end")));
}
