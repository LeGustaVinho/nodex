use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn f32_suffix_is_explained_on_literal_without_confusing_other_literals() {
    let source = "fn f() { let value = 1.5f32; let wide = 1.5f64; let text = \"1.5f32\"; }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Floating-point literal with an f32 suffix")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[sites[0].span.start_byte as usize..sites[0].span.end_byte as usize], "1.5f32");
    assert!(matches!(&sites[0].content, ReadingContent::Detail { text, .. } if text.contains("32-bit") && text.contains("suffix f32") && text.contains("rounding")));
}
