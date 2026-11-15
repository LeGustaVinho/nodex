use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn raw_c_strings_explain_literal_backslashes_and_nul_in_flow_and_constants() {
    let source = r###"const C: &core::ffi::CStr = cr"hello\n"; fn f() { let raw = cr"hello\n"; let processed = c"hello\n"; let hash = cr#"hello\n"#; let text = r"hello\n"; }"###;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Raw C string literal")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    let site = sites[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "cr");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("backslash followed by n") && text.contains("appends a terminating NUL") && text.contains("rejects an embedded NUL") && text.contains("&'static core::ffi::CStr") && text.contains("not") ));
    let constant = ir.symbols.iter().find(|symbol| symbol.name == "C").unwrap();
    let item = constant.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Raw C string literal").expect("constant contract");
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "cr");
}
