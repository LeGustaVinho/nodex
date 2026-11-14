use nodex_ir::reading::ReadingContent;

#[test]
fn explains_uninitialized_bindings_without_claiming_they_have_a_value() {
    let source = "fn f(flag: bool) -> i32 { let x: i32; if flag { x = 1; } else { x = 2; } x } fn g() { let y: i32 = 3; }";
    let ir = nodex_lang_rust::lower_source("bindings.rs", source);
    assert!(ir.diagnostics.is_empty(), "{:?}", ir.diagnostics);
    let explanations: Vec<_> = ir.flows.iter().flat_map(|flow| flow.reading.iter().flatten())
        .filter_map(|site| match &site.content {
            ReadingContent::Detail { label, text, .. } if label == "Declare a variable without a value" => Some(text),
            _ => None,
        }).collect();
    assert_eq!(explanations.len(), 1);
    assert!(explanations[0].contains("x with the declared type i32"));
    assert!(explanations[0].contains("every reachable path before its value is read"));
}
