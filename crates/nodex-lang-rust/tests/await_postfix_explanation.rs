use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn postfix_await_explains_conversion_polling_and_output_without_erasing_existing_detail() {
    let source = "async fn run(future: impl std::future::Future<Output = i32>) -> i32 { let result = future.await; result }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "run").unwrap();
    let detail = flow.reading.iter().flatten().find(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Await an asynchronous result"))
        .unwrap_or_else(|| panic!("{:?}", flow.reading));
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], ".");
    assert!(matches!(&detail.content, ReadingContent::Detail { text, .. }
        if text.contains("Evaluate future first") && text.contains("IntoFuture")
        && text.contains("pending") && text.contains("output becomes")
        && text.contains("does not by itself start a thread")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Await a result")));
}
