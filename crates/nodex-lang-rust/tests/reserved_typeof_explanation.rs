use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn plain_reserved_typeof_is_explained_but_raw_names_and_text_are_not() {
    let source = "fn f(y: i32) { let x: typeof(y) = y; let r#typeof = 1; let s = \"typeof\"; /* typeof */ } fn typeof() {} fn r#typeof() {}";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Reserved typeof keyword")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    assert_eq!(items[0].span.start_byte as usize, source.find("typeof(y)").unwrap());
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "typeof");
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. } if text.contains("reserved Rust keyword") && text.contains("not an available type-query operator") && text.contains("r#typeof")));
    let symbol = file.symbols.iter().find(|symbol| symbol.name == "typeof").unwrap();
    assert!(symbol.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Reserved typeof keyword"));
    let raw = file.symbols.iter().find(|symbol| symbol.name == "r#typeof").unwrap();
    assert!(!raw.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Reserved typeof keyword"));
}
