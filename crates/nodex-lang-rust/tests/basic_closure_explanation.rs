use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_pipe_parameters_and_expression_result_without_running_the_body_at_creation() {
    let text = "fn demo() { let f = |x| x + 1; consume(|n| n + 1); } fn other() { let z = || 1; let typed = |x: i32| x + 1; }";
    let ir = lower_source("src/lib.rs", text);
    let flow = ir.flows.iter().find(|flow| flow.name == "demo").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Closure parameters and result")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&text[site.span.start_byte as usize..site.span.end_byte as usize], "|");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("argument supplied") && text.contains("does not run its body")
                && text.contains("call's result") && text.contains("infer") && text.contains("capture")));
    }
    let other = ir.flows.iter().find(|flow| flow.name == "other").unwrap();
    assert!(!other.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Closure parameters and result")));
}
