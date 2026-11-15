use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_decimal_digit_separators_in_flow_and_const() {
    let source = "const BIG: u32 = 1_000_000; fn f() { let n = 1_000_000; let s = 1_000u32; let plain = 1000; let x = 0x1_00; let text = \"1_000\"; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content { ReadingContent::Detail { label, text, .. } if label == "Decimal digit separators" => Some((site, text)), _ => None }).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    assert!(details.iter().all(|(site, _)| &source[site.span.start_byte as usize..site.span.end_byte as usize] == "_"));
    assert!(details.iter().any(|(_, text)| text.contains("1_000_000 denotes the same decimal integer as 1000000") && text.contains("inference")));
    let constant = ir.symbols.iter().find(|s| s.name == "BIG").unwrap();
    assert!(constant.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Decimal digit separators" && item.text.contains("1000000")));
}
