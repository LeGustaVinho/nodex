use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn negative_inclusive_pattern_is_explained_with_both_negative_endpoints() {
    let source = "fn f(n: i32) { let _range = -5..=-1; match n { -5..=-1 => (), _ => () } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Match an inclusive range")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    let item = sites[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "..=");
    let ReadingContent::Detail { text, .. } = &item.content else { unreachable!() };
    for phrase in ["-5..=-1", "at least -5", "at most -1", "includes both endpoints", "not a loop"] { assert!(text.contains(phrase), "{text}"); }
}
