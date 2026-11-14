use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn named_positional_pattern_explains_order_without_guessing_resolution() {
    let source = "struct Color(u8,u8,u8); fn f(c: Color) { let Color(r,g,b) = c; use_value(r,g,b); } fn g(pair: (u8,u8)) { let (a,b) = pair; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let item = flow.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Match named positional fields")).expect("positional pattern explanation on flow");
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "(");
    let ReadingContent::Detail { text, .. } = &item.content else { unreachable!() };
    assert!(text.contains("fields by position"));
    assert!(text.contains("field 0"));
    assert!(text.contains("does not establish"));
    let other = ir.flows.iter().find(|f| f.name == "g").unwrap();
    assert!(!other.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Match named positional fields")));
}
