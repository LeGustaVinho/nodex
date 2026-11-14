use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn async_move_closure_explains_future_and_capture_without_assuming_copy() {
    let source = "fn f(text: String) { let fun = async move |x: usize| { text.len() + x }; let ordinary = move |x: usize| x + 1; let async_ref = async |x: usize| x + 1; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Create an async move closure")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], "move");
    assert!(matches!(&facts[0].content, ReadingContent::Detail { text, .. }
        if text.contains("copied if its type implements Copy") && text.contains("otherwise it is moved")
        && text.contains("Calling the closure returns a future") && text.contains("does not by itself schedule")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label.starts_with("Create function fun"))));
}
