use nodex_ir::{Concept, reading::ReadingContent};
use nodex_lang_rust::lower_source;

#[test]
fn open_inclusive_pattern_is_visible_on_match_not_range_expression() {
    let source = "fn f(n: i32) { match n { ..=0 => (), _ => () } } fn g() { let r = ..=0; } fn h(n: i32) { match n { 0..=10 => (), _ => () } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Pattern at or below an inclusive limit")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    let item = items[0];
    assert_eq!(flow.nodes.iter().find(|node| node.id == item.node).unwrap().concept, Concept::Match);
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "..=");
    assert!(matches!(&item.content, ReadingContent::Detail { text, .. } if text.contains("less than or equal to 0") && text.contains("0 itself is included") && text.contains("No lower endpoint") && text.contains("does not construct or iterate a range")));
    for name in ["g", "h"] {
        let other = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        assert!(!other.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Pattern at or below an inclusive limit")));
    }
}
