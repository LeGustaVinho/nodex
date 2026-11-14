use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_order_scope_and_failure_of_if_let_chain_in_flow() {
    let source = "fn f(a: Option<i32>, b: Option<i32>) { if let Some(x) = a && let Some(y) = b { use_values(x, y); } else { fallback(); } } fn ordinary(a: bool, b: bool) { if a && b {} }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let detail = flow.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Match conditions in sequence")).expect("chain explanation in flow");
    let ReadingContent::Detail { text, .. } = &detail.content else { unreachable!() };
    assert!(text.contains("left to right") && text.contains("first failure"));
    assert!(text.contains("available to later conditions") && text.contains("not available in else"));
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "&&");
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Steps { label, steps } if label == "Check each condition in order" && steps.len() == 3)));
    let ordinary = ir.flows.iter().find(|flow| flow.name == "ordinary").unwrap();
    assert!(!ordinary.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Match conditions in sequence")));
}

#[test]
fn pattern_then_boolean_condition_is_explained_in_order() {
    let source = "fn f(a: Option<i32>) { if let Some(x) = a && x > 0 { consume(x); } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let steps = flow.reading.iter().flatten().find_map(|site| match &site.content {
        ReadingContent::Steps { label, steps } if label == "Check each condition in order" => Some(steps),
        _ => None,
    }).expect("condition sequence in flow");
    assert!(steps.len() >= 2);
    assert!(steps[0].action.contains("Some(x)"));
    assert!(steps[1].action.contains("x > 0"));
}
