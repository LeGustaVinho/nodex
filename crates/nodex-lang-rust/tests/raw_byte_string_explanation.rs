use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_raw_bytes_without_confusing_processed_or_nonbyte_strings() {
    let source = r##"fn f() { let raw = br"hello\n"; let normal = b"hello\n"; let text = r"hello\n"; }"##;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Raw byte string literal")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    let site = details[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "br");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("backslash followed by the letter n") && text.contains("ASCII") && text.contains("&'static [u8; N]") && text.contains("does not append a zero byte")));
}
