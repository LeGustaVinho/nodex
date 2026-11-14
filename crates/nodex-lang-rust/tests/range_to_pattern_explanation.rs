use nodex_ir::{Concept, reading::ReadingContent};
use nodex_lang_rust::lower_source;

#[test]
fn exclusive_open_start_pattern_is_explained_on_match_without_confusing_ranges() {
    let source = "fn f(x: i32) -> bool { match x { ..10 => true, ..=10 => false, _ => false } } fn g() { let r = ..10; } fn h(x: u8) -> bool { match x { 1..10 => true, _ => false } }";
    let ir = lower_source("src/lib.rs", source);
    let f = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = f.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Pattern below an exclusive limit")).collect();
    assert_eq!(sites.len(), 1, "{:?}", f.reading);
    let site = sites[0];
    assert_eq!(f.nodes.iter().find(|n| n.id == site.node).unwrap().concept, Concept::Match);
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "..");
    let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
    assert!(text.contains("strictly less than 10"));
    assert!(text.contains("endpoint 10 itself is excluded"));
    assert!(text.contains("No lower endpoint is written"));
    assert!(text.contains("representable by the matched type"));
    for name in ["g", "h"] {
        let flow = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        assert!(!flow.reading.iter().flatten().any(|site| matches!(&site.content,
            ReadingContent::Detail { label, .. } if label == "Pattern below an exclusive limit")));
    }
}
