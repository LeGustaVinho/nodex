use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn debug_format_explanation_is_conditional_and_visible_with_generic_macro_detail() {
    let source = r#"macro_rules! format { ($($t:tt)*) => { 0 }; } fn f(value: i32) { let s = format!("{:?}", value); let p = println!("{:?}", value); let n = format!("{}", value); }"#;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Possible Debug formatting")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    let site = facts[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "?");
    let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
    assert!(text.contains("If format! resolves"));
    assert!(text.contains("new owned String instead of printing"));
    assert!(text.contains("Debug representation of value"));
    assert!(text.contains("same name"));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Invoke a macro with parentheses")));
}
