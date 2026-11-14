use nodex_ir::{Access, reading::ReadingContent};
use nodex_lang_rust::lower_source;

#[test]
fn explicit_ref_mut_is_explained_and_keeps_pattern_access() {
    let source = "struct Point { x: i32 } fn f(mut value: Option<i32>, mut p: Point) { if let Some(ref mut item) = value { *item += 1; } let Point { ref mut x } = p; *x += 1; } fn shared(v: Option<i32>) { if let Some(ref item) = v {} }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, text, .. } if label == "Bind with an exclusive reference" && text.contains("rather than moving") && text.contains("conflicting access"))).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    assert!(details.iter().all(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == "ref"));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Pattern { parts, .. } if parts.iter().any(|part| part.binding.as_deref() == Some("item") && part.access == Some(Access::BorrowMut)))));
    let shared = ir.flows.iter().find(|flow| flow.name == "shared").unwrap();
    assert!(!shared.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Bind with an exclusive reference")));
}
