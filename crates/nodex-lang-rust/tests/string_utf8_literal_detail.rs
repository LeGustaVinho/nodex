use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn ordinary_text_literal_is_explained_in_flow_and_constant_contract() {
    let source = r##"const TEXT: &str = "hello"; fn f() { let text = "hello"; let bytes = b"hello"; let c = c"hello"; let raw = r"hello"; }"##;
    let ir = lower_source("src/lib.rs", source);
    let constant = ir.symbols.iter().find(|symbol| symbol.name == "TEXT").unwrap();
    let item = constant.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "UTF-8 string literal").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "\"");
    assert!(item.text.contains("&'static str") && item.text.contains("UTF-8") && item.text.contains("byte length can differ") && item.text.contains("not an owned"));
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "UTF-8 string literal")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "\"");
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. } if text.contains("&'static str") && text.contains("UTF-8") && text.contains("escapes") && text.contains("not automatically append a C NUL")));
}
