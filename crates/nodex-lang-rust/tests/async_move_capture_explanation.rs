use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn async_move_explains_capture_and_lazy_execution_on_the_flow() {
    let source = "fn f(s: String, n: i32) { let a = async move { consume(s, n); }; let b = async { 1 }; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let found: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Capture values in an async future"
    )).collect();
    assert_eq!(found.len(), 1, "{:?}", flow.reading);
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Create a future"
    )), "{:?}", flow.reading);
    let site = found[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "move");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("copied if") && text.contains("otherwise it is moved") && text.contains("reference") && text.contains("polled") && text.contains("does not run the body")));
}
