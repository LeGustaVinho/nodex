use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn panic_message_explains_unwind_and_abort_only_if_macro_is_standard() {
    let source = r#"macro_rules! panic { ($x:literal) => { () }; } fn f() { panic!("message"); } fn other() { custom!("message"); }"#;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Possible panic")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    let site = facts[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "\"message\"");
    let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
    assert!(text.contains("If panic! resolves"));
    assert!(text.contains("with unwinding"));
    assert!(text.contains("destructors run"));
    assert!(text.contains("With abort"));
    assert!(text.contains("without unwinding"));
    assert!(text.contains("might even return normally"));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Invoke a macro with parentheses")));
    let other = ir.flows.iter().find(|flow| flow.name == "other").unwrap();
    assert!(!other.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Possible panic")));
}
