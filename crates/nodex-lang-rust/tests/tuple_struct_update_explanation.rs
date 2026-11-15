use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn numbered_field_update_explains_index_and_base() {
    let source = "fn f(old: Pair) { let p = Pair { 1: 3, ..old }; let q = Pair { 1: 3, 0: 2 }; let r = Point { x: 3, ..old }; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|s| matches!(&s.content, ReadingContent::Detail { label, .. } if label == "Update a numbered field")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    let site = sites[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "1");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("zero-based index") && text.contains("expression 3") && text.contains("..old") && text.contains("copied") && text.contains("moved") && text.contains("declaration determines")));
}
