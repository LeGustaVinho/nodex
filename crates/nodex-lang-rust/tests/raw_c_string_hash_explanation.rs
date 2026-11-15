use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_hash_delimited_raw_c_string_without_confusing_other_literals() {
    let source = "const NAME: &std::ffi::CStr = cr#\"hello\"#; fn f() { let c = cr#\"hello\\n\"#; let more = cr##\"quote \"# remains\"##; let raw = cr\"plain\"; let bytes = br#\"hi\"#; let text = r#\"hi\"#; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Raw C string with hash delimiters")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    assert!(facts.iter().all(|site| matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("backslashes are literal") && text.contains("same number of hashes")
        && text.contains("terminating zero byte") && text.contains("must not contain a zero byte") && text.contains("&'static CStr"))));
    assert!(facts.iter().any(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == "cr#"));
    let constant = ir.symbols.iter().find(|symbol| symbol.name == "NAME").unwrap();
    assert!(constant.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Raw C string with hash delimiters" && item.text.contains("terminating zero byte")));
}
