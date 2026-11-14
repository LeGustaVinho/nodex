use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn byte_literal_is_explained_in_flow_and_constant_without_confusing_char_or_byte_string() {
    let source = r#"const C: u8 = b'A'; fn run() { let letter = b'A'; consume(b'\xFF'); let character = 'A'; let bytes = b"A"; }"#;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "run").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Single-byte literal")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in &sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "b");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("one u8 byte") && text.contains("Unicode scalar") && text.contains("must be ASCII") && text.contains("no runtime")));
    }
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("value 65"))));
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("value 255"))));
    let constant = ir.symbols.iter().find(|symbol| symbol.name == "C").unwrap();
    let item = constant.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Single-byte literal").expect("constant byte literal");
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "b");
    assert!(item.text.contains("value 65"));
}
