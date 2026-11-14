use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn async_closure_creation_and_future_timing_are_explained_in_parent_flow() {
    let source = "fn build() { let f = async |x: i32| { x + 1 }; let g = async move |x: i32| { x + 2 }; let sync = |x: i32| { x }; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "build").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content {
        ReadingContent::Detail { label, text, .. } if label == "Create an async closure" => Some((site, text)),
        _ => None,
    }).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    assert!(details.iter().all(|(site, text)| &source[site.span.start_byte as usize..site.span.end_byte as usize] == "async" && text.contains("creates a future") && text.contains("polled") && text.contains("does not automatically schedule")));
    assert_eq!(details.iter().filter(|(_, text)| text.contains("`move` keyword")).count(), 1);
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label.starts_with("Create function"))), "existing closure detail should remain visible");
}
