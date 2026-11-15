use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn unicode_char_escape_is_explained_without_confusing_strings_or_plain_characters() {
    let source = r##"fn f() { let happy = '\u{1F600}'; let same = '\u{1f_600}'; let plain = 'x'; let string = "\u{1F600}"; }"##;
    assert!(!nodex_lang_rust::parse(source).unwrap().root_node().has_error());
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Unicode character escape")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in sites {
        let written = &source[site.span.start_byte as usize..site.span.end_byte as usize];
        assert!(written == r"\u{1F600}" || written == r"\u{1f_600}", "{written}");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("U+1F600") && text.contains("one Unicode scalar") && text.contains("not necessarily one visible character") && text.contains("surrogate")));
    }
}
