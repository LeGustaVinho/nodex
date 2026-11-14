use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn match_alternatives_and_fallback_are_both_explained() {
    let source = "enum E { A, B, C } use E::{A, B}; fn f(x: E) -> i32 { match x { A | B => 1, _ => 0 } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details = flow.reading.as_ref().unwrap();
    assert!(details.iter().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. }
        if label == "Alternative patterns" && text.contains("any of the written alternative patterns") && text.contains("same names"))), "{details:?}");
    assert!(details.iter().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. }
        if label == "Choose a matching arm" && text.contains("first pattern") && text.contains("supplies the value") && text.contains("cover every possible value") && text.contains("`_`"))), "{details:?}");
}
