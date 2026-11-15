use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn non_final_match_block_may_omit_comma_before_next_arm() {
    let source = "fn f(x: i32) { match x { 0 => {} _ => {} } } fn g(x: i32) { match x { 0 => {}, 1 => 1, _ => 2 } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Match arm without a comma")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    let site = sites[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "}");
    let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
    for phrase in ["comma", "omitted", "separate arm", "first arm", "also allowed", "expression without a block", "final arm"] { assert!(text.contains(phrase), "{text}"); }
    let other = ir.flows.iter().find(|flow| flow.name == "g").unwrap();
    assert!(!other.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Match arm without a comma")));
}
