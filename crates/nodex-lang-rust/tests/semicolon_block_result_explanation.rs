use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_final_semicolon_as_discard_and_conditional_unit() {
    let source = "fn f() { let a = { side(); 42; }; let b = { side(); 42 }; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let found: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "A semicolon makes the block yield unit"
    )).collect();
    assert_eq!(found.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[found[0].span.start_byte as usize..found[0].span.end_byte as usize], ";");
    assert!(matches!(&found[0].content, ReadingContent::Detail { text, .. } if text.contains("discard") && text.contains("normally") && text.contains("does not produce unit")));
}
