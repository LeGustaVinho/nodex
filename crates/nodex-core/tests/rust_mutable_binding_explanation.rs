use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn mutable_binding_explains_initialization_and_reassignment() {
    let source = "fn demo() { let mut x = 10; x = 20; let y = 1; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "demo").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| {
        matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Create a changeable variable")
    }).collect();
    assert_eq!(details.len(), 1);
    let site = details[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "mut");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("x") && text.contains("initializes") && text.contains("assigning a new value")));
}
