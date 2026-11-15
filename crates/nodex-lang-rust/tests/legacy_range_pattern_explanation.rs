use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn legacy_inclusive_pattern_explains_bounds_and_edition() {
    let source = "fn f(n: u8, pair: (u8, u8)) { match n { 1...5 => (), _ => () } match pair { (0...2, _) => (), _ => () } match n { 1..=5 => (), _ => () } let s = \"1...5\"; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Legacy inclusive range pattern")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "...");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("including both ends") && text.contains("2015 and 2018") && text.contains("error in Rust 2021 and later") && text.contains("1..=5") && text.contains("does not establish")));
    }
}
