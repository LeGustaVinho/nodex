use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn named_inclusive_range_explains_bounds_without_assuming_resolution() {
    let source = "fn f(n: i32) { match n { LOW..=HIGH => (), _ => () }; let r = LOW..=HIGH; match n { 1..=10 => (), _ => () }; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|s| matches!(&s.content, ReadingContent::Detail { label, .. } if label == "Match between named bounds")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[sites[0].span.start_byte as usize..sites[0].span.end_byte as usize], "LOW");
    assert!(matches!(&sites[0].content, ReadingContent::Detail { text, .. } if text.contains("LOW..=HIGH") && text.contains("both included") && text.contains("allowed constant values") && text.contains("does not establish") && text.contains("following `_`")));
    assert!(flow.reading.iter().flatten().any(|s| matches!(&s.content, ReadingContent::Detail { label, .. } if label == "Match an inclusive range")));
}
