use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explicit_literal_const_argument_is_visible_without_guessing_resolution() {
    let source = "fn demo() { foo::<42>(); foo::<u32>(); foo(42); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "demo").unwrap();
    let matches: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Explicit constant generic argument")).collect();
    assert_eq!(matches.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[matches[0].span.start_byte as usize..matches[0].span.end_byte as usize], "42");
    assert!(matches!(&matches[0].content, ReadingContent::Detail { text, .. }
        if text.contains("not as a run-time argument") && text.contains("does not identify the declaration")));
}
