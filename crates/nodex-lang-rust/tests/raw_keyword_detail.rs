use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn raw_identifier_uses_are_explained_without_mistaking_strings_or_comments() {
    let source = "fn f(r#type: i32) { let r#match = r#type; consume(r#match); let s = r#\"r#type\"#; /* r#match */ }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Use a raw identifier")).collect();
    let expected_type = source.find("= r#type").unwrap() + 2;
    let expected_match = source.find("consume(r#match)").unwrap() + "consume(".len();
    for start in [expected_type, expected_match] {
        let site = sites.iter().find(|site| site.span.start_byte as usize == start).unwrap_or_else(|| panic!("{start}: {:?}", flow.reading));
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("prefix is syntax") && text.contains("does not determine") && text.contains("r#self") && text.contains("r#_")));
    }
    let raw_string = source.find("r#\"r#type\"#").unwrap();
    assert!(!sites.iter().any(|site| (site.span.start_byte as usize) >= raw_string));
}
