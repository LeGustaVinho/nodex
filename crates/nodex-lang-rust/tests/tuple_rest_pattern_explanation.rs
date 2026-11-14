use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_tuple_rest_from_both_ends_without_confusing_slice_or_range() {
    let source = "fn f(t: (u8,u8,u8,u8), s: &[u8]) { let (x, .., y) = t; match t { (a, .., b) => consume(a,b) } let [first, ..] = s else { return; }; let range = 1..3; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Skip middle tuple positions")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for site in facts {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "..");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("including zero") && text.contains("from the start") && text.contains("from the end") && text.contains("not a range expression")));
    }
}
