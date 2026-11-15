use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn unicode_local_name_is_explained_without_treating_it_as_keyword() {
    let source = "fn f() { let 東京 = 1; let ascii = 2; let café = ascii; let _ = 東京; let text = \"東京\"; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Unicode local identifier")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    assert!(sites.iter().any(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == "東京"));
    assert!(sites.iter().any(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == "café"));
    assert!(matches!(&sites[0].content, ReadingContent::Detail { text, .. } if text.contains("Unicode") && text.contains("not a Rust keyword") && text.contains("normalized to NFC") && text.contains("type is inferred") && text.contains("Without `mut`")));
}
