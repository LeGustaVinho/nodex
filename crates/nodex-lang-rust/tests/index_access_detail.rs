use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn index_detail_covers_reads_and_writes_without_guessing_custom_types() {
    let source = "fn read(values: &[i32], i: usize) { let x = values[i]; let all = &values[..]; } fn write(values: &mut [i32], i: usize) { values[i] = 1; } fn custom(value: Custom, i: usize) { let x = value[i]; }";
    let file = lower_source("src/lib.rs", source);
    for name in ["read", "write", "custom"] {
        let flow = file.flows.iter().find(|flow| flow.name == name).unwrap();
        let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Index a value")).collect();
        assert_eq!(sites.len(), 1, "{name}: {:?}", flow.reading);
        assert_eq!(&source[sites[0].span.start_byte as usize..sites[0].span.end_byte as usize], "[");
        assert!(matches!(&sites[0].content, ReadingContent::Detail { text, .. } if text.contains("If the operand is an array") && text.contains("out-of-bounds access panics") && text.contains("Index or IndexMut") && text.contains("does not prove") && text.contains("mutable access")));
    }
}
