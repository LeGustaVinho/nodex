use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn one_tuple_binding_explains_comma_without_grouped_or_multiple_patterns() {
    let source = "fn f() { let (a,): (i32,) = (1,); let (b) = 2; let (c, d) = (3, 4); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "One-element tuple pattern")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], ",");
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. } if text.contains("comma makes this a one-element tuple pattern") && text.contains("only groups") && text.contains("type and in the value expression") && text.contains("copied or moved")));
}
