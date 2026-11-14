use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn shared_ref_pattern_is_explained_without_confusing_mutable_or_reference_patterns() {
    let source = "fn read(value: Option<String>) { if let Some(ref text) = value { use_value(text); } } fn other(mut v: Option<i32>, r: &i32) { if let Some(ref mut x) = v { *x += 1; } let &y = r; }";
    let ir = lower_source("src/lib.rs", source);
    let read = ir.flows.iter().find(|f| f.name == "read").unwrap();
    let details: Vec<_> = read.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Bind a shared reference")).collect();
    assert_eq!(details.len(), 1, "{:?}", read.reading);
    assert_eq!(&source[details[0].span.start_byte as usize..details[0].span.end_byte as usize], "ref");
    let ReadingContent::Detail { text, .. } = &details[0].content else { unreachable!() };
    assert!(text.contains("instead of moving"));
    assert!(text.contains("shared reference"));
    let other = ir.flows.iter().find(|f| f.name == "other").unwrap();
    assert!(!other.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Bind a shared reference")));
}
