use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_one_hash_raw_string_without_matching_other_literal_forms() {
    let source = r###"fn f() { let s = r#"He said "hi" and \n"#; let z = r"plain"; let m = r##"many"##; let b = br#"bytes"#; }"###;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "One-hash raw string"
    )).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[details[0].span.start_byte as usize..details[0].span.end_byte as usize], "r#\"");
    assert!(matches!(&details[0].content, ReadingContent::Detail { text, .. }
        if text.contains("double quotes") && text.contains("not interpreted") && text.contains("&'static str")));
}
