use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn binding_and_alternatives_are_both_explained_in_match() {
    let source = "fn f(x: Option<i32>) { match x { v @ (Some(1) | Some(2)) => (), _ => () } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content { ReadingContent::Detail { label, text, .. } => Some((label.as_str(), text.as_str())), _ => None }).collect();
    assert!(details.iter().any(|(label, text)| *label == "Bind the value while checking a pattern" && text.contains("entire value") && text.contains("resolved in context")), "{details:?}");
    assert!(details.iter().any(|(label, text)| *label == "Alternative patterns" && text.contains("any of the written alternative") && text.contains("same names")), "{details:?}");
}
