use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_explicit_closure_parameter_and_preserves_closure_reading() {
    let source = "fn f() { let double = |x: i32| x * 2; let inferred = |x| x + 1; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Typed closure parameter")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[details[0].span.start_byte as usize..details[0].span.end_byte as usize], ":");
    assert!(matches!(&details[0].content, ReadingContent::Detail { text, .. } if text.contains("x: i32") && text.contains("not a cast") && text.contains("when the closure is called")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label.starts_with("Create function double"))));
}
