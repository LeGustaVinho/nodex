use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn distinguishes_rest_in_struct_and_slice_patterns_from_full_range() {
    let source = "struct Point { x: i32, y: i32 } fn f(p: Point, xs: &[i32]) { let Point { x, .. } = p; match xs { [first, ..] => use_it(first), _ => () } let all = ..; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Ignore remaining pattern parts"
    )).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    assert!(details.iter().all(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == ".."));
    assert!(details.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("remaining fields"))));
    assert!(details.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("at least 1 element"))));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Range has no bounds"
    )), "{:?}", flow.reading);
}
