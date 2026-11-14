use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn async_blocks_explain_lazy_future_and_result_on_the_flow() {
    let source = "async fn f() { let delayed = async { 1 }; let moved = async move { 2 }; let result = delayed.await; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Async block creates a future")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "async");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("does not run its body") && text.contains("polled") && text.contains("final expression") && text.contains("effects already performed")));
    }
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Create a future")));
}
