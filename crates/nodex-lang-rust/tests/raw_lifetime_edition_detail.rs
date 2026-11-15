use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_edition_for_raw_lifetime_and_loop_label_in_context() {
    let source = "fn f<'r#loop>(x: &'r#loop str) -> &'r#loop str { 'r#loop: loop { break 'r#loop; } x }";
    let ir = lower_source("src/lib.rs", source);
    let function = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let facts: Vec<_> = function.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Raw lifetime edition").collect();
    assert!(!facts.is_empty(), "{:?}", function.rust_contract);
    assert!(facts.iter().all(|item| item.text.contains("2021") && item.text.contains("earlier editions") && item.text.contains("does not establish")));
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let detail = flow.reading.iter().flatten().find(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Raw label and edition")).unwrap_or_else(|| panic!("{:?}", flow.reading));
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "r#");
    assert!(matches!(&detail.content, ReadingContent::Detail { text, .. } if text.contains("2021") && text.contains("earlier editions") && text.contains("control-flow target") && text.contains("not a lifetime")));
}
