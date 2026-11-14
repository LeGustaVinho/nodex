use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_raw_mut_address_at_the_flow_site() {
    let text = "fn f(mut x: i32) { let a = &raw mut x; let b = &raw const x; let c = &mut x; }";
    let ir = lower_source("src/lib.rs", text);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Take a mutable raw address")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    assert_eq!(&text[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "raw");
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. } if text.contains("*mut T") && text.contains("does not create a Rust reference")));
}
