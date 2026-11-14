use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_unit_type_in_signature_and_value_in_flow() {
    let text = "fn finish() -> () { let value: () = (); value } fn other() -> i32 { 1 }";
    let ir = lower_source("src/lib.rs", text);
    let function = ir.symbols.iter().find(|symbol| symbol.name == "finish").unwrap();
    let details: Vec<_> = function.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Unit type").collect();
    assert_eq!(details.len(), 1, "{details:?}");
    assert!(details[0].text.contains("exactly one value"));
    assert_eq!(&text[details[0].span.start_byte as usize..details[0].span.end_byte as usize], "()");
    let flow = ir.flows.iter().find(|flow| flow.name == "finish").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, text, .. } if label == "Unit value" && text.contains("sole value"))));
    let other = ir.symbols.iter().find(|symbol| symbol.name == "other").unwrap();
    assert!(!other.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Unit type")));
}
