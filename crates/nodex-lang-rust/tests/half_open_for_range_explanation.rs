use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn for_range_explains_exclusive_end_and_empty_range_without_confusing_other_loops() {
    let source = "fn run() { for i in 1..4 { let _ = i; } for i in 1..=4 { let _ = i; } for i in [1, 2] { let _ = i; } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "run").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Iterate over a half-open range")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    let site = sites[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "in");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("includes its start and excludes its end") && text.contains("stopping before 4")
        && text.contains("greater than or equal to the end") && text.contains("break")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Repeat for each iterator item")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Range stops before the end")));
}
