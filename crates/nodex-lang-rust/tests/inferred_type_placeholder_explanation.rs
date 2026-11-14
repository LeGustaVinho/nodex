use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn distinguishes_type_placeholder_from_discard_pattern_and_explains_generic_argument() {
    let source = "fn run() { let x: _ = 42; let values: Vec<_> = vec![1]; let _: i32 = 0; let value = make::<_>(); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "run").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|s| matches!(&s.content, ReadingContent::Detail { label, .. } if label == "Infer a type or generic argument")).collect();
    assert_eq!(sites.len(), 3, "{:?}", flow.reading);
    assert!(sites.iter().any(|s| matches!(&s.content, ReadingContent::Detail { text, .. } if text.contains("initializer") && text.contains("compile time"))));
    assert_eq!(sites.iter().filter(|s| matches!(&s.content, ReadingContent::Detail { text, .. } if text.contains("generic argument"))).count(), 2);
    assert!(flow.reading.iter().flatten().any(|s| matches!(&s.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
}
