use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn block_initializer_explains_tail_and_semicolon_without_erasing_existing_detail() {
    let source = "fn f() { let x = { let y = 2; y + 1 }; let empty = { 7; }; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content {
        ReadingContent::Detail { label, text, .. } if label == "Initialize from a block" => Some(text),
        _ => None,
    }).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    assert!(details.iter().any(|text| text.contains("y + 1") && text.contains("without an ending semicolon")));
    assert!(details.iter().any(|text| text.contains("unit value ()") && text.contains("discards")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Value from the final expression")));
}
