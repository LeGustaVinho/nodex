use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn labeled_loop_explains_targeting_across_nesting() {
    let source = "fn f() { 'outer: loop { loop { if ready() { break 'outer; } continue; } } loop { break; } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Name this loop")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    let site = details[0];
    assert!(source[site.span.start_byte as usize..site.span.end_byte as usize].starts_with("'outer"));
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("break 'outer") && text.contains("continue 'outer") && text.contains("innermost loop") && text.contains("not a lifetime")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Repeat the loop body")));
}
