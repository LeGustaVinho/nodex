use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn computed_const_generic_argument_is_visible_in_the_flow() {
    let text = "fn run() { foo::<{ 2 + 2 }>(); foo::<42>(); }";
    let ir = lower_source("src/lib.rs", text);
    let flow = ir.flows.iter().find(|flow| flow.name == "run").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Computed const generic argument")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. }
        if text.contains("{ 2 + 2 }") && text.contains("compile time") && text.contains("const generic")));
}
