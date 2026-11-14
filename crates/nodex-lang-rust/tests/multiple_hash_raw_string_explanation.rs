use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_multiple_hash_delimiters_in_flow_without_matching_single_hash() {
    let source = r###"fn show() { let text = r##"text"#more\n"##; let plain = r#"abc"#; }"###;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "show").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Raw string with multiple hashes")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    let site = sites[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "r##\"");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("followed by 2 hash") && text.contains("fewer than 2")
        && text.contains("not interpreted") && text.contains("&'static str")));
}
