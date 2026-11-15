use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn right_shift_assignment_explains_primitive_and_custom_behavior() {
    let source = "fn f(mut x: i32, n: u32, mut custom: Value) { x >>= n; custom >>= n; x <<= n; x >> n; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|s| matches!(&s.content, ReadingContent::Detail { label, .. } if label == "Shift right and assign")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], ">>=");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("primitive integers") && text.contains("arithmetic right shift") && text.contains("ShrAssign") && text.contains("does not establish") && text.contains("produces `()`") && text.contains("right side is evaluated first")));
    }
}
