use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn unary_dereference_is_explained_in_read_and_assignment_without_treating_multiplication_as_deref() {
    let source = "fn f(reference: &mut i32, a: i32, b: i32) { let x = *reference; *reference = 7; let y = a * b; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Dereference a value")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "*");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("reference") && text.contains("raw pointer") && text.contains("unsafe") && text.contains("cannot be determined")));
    }
}
