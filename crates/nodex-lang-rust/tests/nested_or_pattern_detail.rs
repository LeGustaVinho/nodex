use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn nested_or_pattern_is_explained_in_match() {
    let source = "fn f(x: Option<i32>) { match x { Some(1 | 2) => (), _ => () } }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Alternative patterns")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[details[0].span.start_byte as usize..details[0].span.end_byte as usize], "|");
    assert!(matches!(&details[0].content, ReadingContent::Detail { text, .. } if text.contains("1 | 2") && text.contains("same position") && text.contains("same names")));
}
