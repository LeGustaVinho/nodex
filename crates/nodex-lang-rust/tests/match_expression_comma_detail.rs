use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn nonfinal_expression_arm_comma_is_explained_at_separator() {
    let source = "fn f(x: i32) -> i32 { match x { 0 => 1, _ => 2 } } fn g(x: i32) { match x { 0 => {} _ => {} } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content { ReadingContent::Detail { label, text, .. } if label == "Comma between match arms" => Some((site,text)), _ => None }).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[sites[0].0.span.start_byte as usize..sites[0].0.span.end_byte as usize], ",");
    for phrase in ["non-final", "required", "final arm", "omit", "block", "only that arm"] { assert!(sites[0].1.contains(phrase)); }
    let flow = ir.flows.iter().find(|f| f.name == "g").unwrap();
    assert!(!flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Comma between match arms")));
}
