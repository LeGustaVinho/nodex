use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn triple_hash_delimiters_are_explained_by_existing_raw_string_coverage() {
    let source = "fn f() { let a = r###\"text\"###; let b = r###\"inside\"##still\\n\"###; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|s| matches!(&s.content,
        ReadingContent::Detail { label, .. } if label == "Raw string with multiple hashes")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "r###\"");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("followed by 3 hash") && text.contains("fewer than 3") && text.contains("not interpreted") && text.contains("&'static str")));
    }
}
