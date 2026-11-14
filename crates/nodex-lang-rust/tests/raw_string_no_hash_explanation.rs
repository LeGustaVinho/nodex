use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn raw_string_without_hashes_explains_literal_backslashes_in_the_flow() {
    let source = r##"fn f() { let raw = r"hello\n"; let hashed = r#"hello\n"#; let ordinary = "hello\n"; let bytes = br"hello\n"; }"##;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Raw string without hashes")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    let site = sites[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "r");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("backslash followed by `n`") && text.contains("not a newline") && text.contains("&'static str") && text.contains("matching `#`")));
}
