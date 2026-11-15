use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_unicode_escape_only_in_ordinary_strings() {
    let source = r##"fn f() { let s = "\u{1F600}"; let literal = "\\u{1F600}"; let raw = r"\u{1F600}"; let c = '\u{1F600}'; }"##;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Unicode escape in a string"
    )).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[details[0].span.start_byte as usize..details[0].span.end_byte as usize], r"\u{1F600}");
    assert!(matches!(&details[0].content, ReadingContent::Detail { text, .. }
        if text.contains("U+1F600") && text.contains("UTF-8") && text.contains("surrogate") && text.contains("raw string")));
}
