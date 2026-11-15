use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn c_string_literal_explains_terminator_and_type_in_flow_and_constant() {
    let source = r##"const C: &core::ffi::CStr = c"hello"; fn f() { let text = c"hello"; let non_utf8 = c"\xff"; let normal = "hello"; let bytes = b"hello"; let raw = cr"hello"; }"##;
    let ir = lower_source("src/lib.rs", source);
    let constant = ir.symbols.iter().find(|symbol| symbol.name == "C").unwrap();
    let item = constant.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "C string literal").expect("constant explanation");
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "c");
    assert!(item.text.contains("&'static core::ffi::CStr"));
    assert!(item.text.contains("automatically appends a zero byte"));
    assert!(item.text.contains("interior NUL"));
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "C string literal")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for detail in details {
        assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "c");
        let ReadingContent::Detail { text, .. } = &detail.content else { unreachable!() };
        assert!(text.contains("not guaranteed to be valid UTF-8"));
        assert!(text.contains("not a raw C pointer"));
    }
}
