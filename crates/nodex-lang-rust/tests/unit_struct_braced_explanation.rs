use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn empty_braced_construction_is_explained_without_assuming_resolution() {
    let source = "struct Unit; struct Empty {} enum Choice { Empty {} } fn f() { let a = Unit {}; let b = Empty { /* no fields */ }; let c = Choice::Empty {}; let d = Unit; let e = Point { x: 1 }; Empty {} = b; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Empty braced value construction")).collect();
    assert_eq!(sites.len(), 3, "{:?}", flow.reading);
    for site in &sites {
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("If ") && text.contains("unit-like struct") && text.contains("bare name") && text.contains("name resolution") && text.contains("does not prove")));
    }
    assert!(sites.iter().any(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == "Unit"));
    assert!(sites.iter().any(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == "Choice::Empty"));
}
