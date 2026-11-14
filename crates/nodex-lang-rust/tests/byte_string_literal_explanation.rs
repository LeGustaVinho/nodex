use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn byte_string_literals_are_explained_in_flow_and_const_contract() {
    let source = r##"const B: &[u8; 5] = b"hello"; fn f() { let bytes = b"hello"; let invalid_utf8 = b"\xff"; let text = "hello"; let byte = b'h'; let raw = br"hello"; }"##;
    let ir = lower_source("src/lib.rs", source);
    let constant = ir.symbols.iter().find(|s| s.name == "B").unwrap();
    let item = constant.rust_contract.as_ref().unwrap().iter().find(|i| i.group == "Byte-string literal").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "b");
    assert!(item.text.contains("&'static [u8; N]"));
    assert!(item.text.contains("five bytes are 104, 101, 108, 108, and 111"));
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Byte-string literal")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for site in facts {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "b");
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        assert!(text.contains("not guaranteed to be valid UTF-8"));
        assert!(text.contains("after escape sequences are processed"));
        assert!(text.contains("does not automatically add a zero terminator"));
    }
}
