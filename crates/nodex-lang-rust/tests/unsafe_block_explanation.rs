use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_unsafe_scope_and_obligations_in_statements_and_values() {
    let source = "fn run(p: *const i32) { unsafe { perform(); } let value = unsafe { *p }; use_value(value); } fn safe() { perform(); }";
    let ir = lower_source("src/lib.rs", source);
    let run = ir.flows.iter().find(|flow| flow.name == "run").unwrap();
    let details: Vec<_> = run.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Unsafe block and safety obligations")).collect();
    assert_eq!(details.len(), 2, "{:?}", run.reading);
    for detail in details {
        assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "unsafe");
        let ReadingContent::Detail { text, .. } = &detail.content else { unreachable!() };
        assert!(text.contains("does not prove"));
        assert!(text.contains("still checks types and borrowing"));
        assert!(text.contains("final expression"));
    }
    let safe = ir.flows.iter().find(|flow| flow.name == "safe").unwrap();
    assert!(!safe.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Unsafe block and safety obligations")));
}
