use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn empty_match_explains_conditional_exhaustiveness() {
    let source = "enum Never {} fn f(never: Never) { match never { /* no arms */ } } fn g(n: i32) { match n { _ => () } }";
    let ir = lower_source("src/lib.rs", source);
    let f = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = f.reading.iter().flatten().filter(|s| matches!(&s.content, ReadingContent::Detail { label, .. } if label == "Match with no arms")).collect();
    assert_eq!(sites.len(), 1, "{:?}", f.reading);
    assert_eq!(&source[sites[0].span.start_byte as usize..sites[0].span.end_byte as usize], "}");
    assert!(matches!(&sites[0].content, ReadingContent::Detail { text, .. } if text.contains("Evaluate never first") && text.contains("no possible values") && text.contains("does not establish") && text.contains("visibility")));
    let g = ir.flows.iter().find(|flow| flow.name == "g").unwrap();
    assert!(!g.reading.iter().flatten().any(|s| matches!(&s.content, ReadingContent::Detail { label, .. } if label == "Match with no arms")));
}
