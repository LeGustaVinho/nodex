use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn block_explains_order_scope_and_final_semicolon_in_flow() {
    let source = "fn f() { let a = { let n = 1; n + 1 }; let b = { let n = 1; n + 1; }; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|r| matches!(&r.content, ReadingContent::Detail { label, .. } if label == "Block as a value")).collect();
    assert_eq!(items.len(), 2, "{:?}", flow.reading);
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "{");
        assert!(matches!(&item.content, ReadingContent::Detail { text, .. } if text.contains("without a semicolon") && text.contains("yields ()") && text.contains("local to its scope")));
    }
}
