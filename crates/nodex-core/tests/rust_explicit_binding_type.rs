use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_explicit_type_on_initialized_binding() {
    let source = "fn example() { let x: i32 = 10; let inferred = 10; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "example").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site|
        matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")
    ).collect();
    assert_eq!(sites.len(), 1);
    assert!(matches!(&sites[0].content, ReadingContent::Detail { text, .. }
        if text.contains("x: i32") && text.contains("not a conversion")));
}
