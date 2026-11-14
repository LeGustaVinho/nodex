use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_hexadecimal_and_suffix_together_in_flow_and_constant_contract() {
    let source = "const MASK: u8 = 0xFF_u8; fn f() { let mask = 0xFF_u8; let lower = 0xff; let decimal = 255u8; let text = \"0xFF_u8\"; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let hex: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Hexadecimal integer literal")).collect();
    assert_eq!(hex.len(), 2, "{:?}", flow.reading);
    assert!(hex.iter().all(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == "0x"));
    assert!(hex.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("digits FF represent 255 in decimal") && text.contains("Underscores") && text.contains("base 16"))));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Integer literal with u8 suffix")
        && &source[site.span.start_byte as usize..site.span.end_byte as usize] == "u8"));
    let mask = ir.symbols.iter().find(|s| s.name == "MASK").unwrap();
    let items = mask.rust_contract.as_ref().unwrap();
    assert!(items.iter().any(|item| item.group == "Hexadecimal integer literal" && item.text.contains("255 in decimal")));
    assert!(items.iter().any(|item| item.group == "Integer literal with u8 suffix"));
}
