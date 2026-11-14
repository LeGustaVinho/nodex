use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn by_value_binding_explains_copy_and_move_without_guessing_type() {
    let source = "fn f(a: String, n: i32) { let b = a; let mut m = n; let literal = 2; let reference = &n; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Bind a value from another name")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for fact in facts {
        let ReadingContent::Detail { text, .. } = &fact.content else { unreachable!() };
        assert!(text.contains("If "));
        assert!(text.contains("implements Copy"));
        assert!(text.contains("moves ownership"));
        assert!(text.contains("reinitialized"));
        assert!(text.contains("cannot decide"));
        assert!(matches!(&source[fact.span.start_byte as usize..fact.span.end_byte as usize], "a" | "n"));
    }
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Introduce an immutable binding")));
}
