use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn let_rest_only_explains_irrefutable_ignored_elements_on_flow() {
    let source = "fn f(array: [u8; 2], slice: &[u8]) { let [..] = array; let [..] = slice; let [head, ..] = array; let _ = array; }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Ignore every element in a sequence pattern")).collect();
    assert_eq!(items.len(), 2, "{:?}", flow.reading);
    for site in items {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "..");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("including when there are zero") && text.contains("irrefutable") && text.contains("no element bindings") && text.contains("does not read or move individual ignored elements")));
    }
}
