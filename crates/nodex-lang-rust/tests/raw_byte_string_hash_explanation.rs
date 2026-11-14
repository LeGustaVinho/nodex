use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_hash_delimited_raw_byte_string_without_treating_other_literals_as_one() {
    let source = r##"fn f() { let bytes = br#"hello\n"#; let text = r#"hello"#; let raw = br"plain"; let regular = b"hello"; }"##;
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Raw byte string with hash delimiters")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[sites[0].span.start_byte as usize..sites[0].span.end_byte as usize], r##"br#"hello\n"#"##);
    assert!(matches!(&sites[0].content, ReadingContent::Detail { text, .. } if text.contains("backslashes are literal") && text.contains("fixed-size byte array") && text.contains("ASCII") && text.contains("same number")));
}
