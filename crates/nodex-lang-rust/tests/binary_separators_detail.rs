use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn binary_separators_explained_in_flow_and_constant() {
    let source = "const MASK: u8 = 0b1010_0110; fn f() { let n = 0b1010_0110; let plain = 0b1010; let s = \"0b1010_0110\"; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|s| matches!(&s.content, ReadingContent::Detail { label, .. } if label == "Binary integer with separators")).collect();
    assert_eq!(items.len(), 1);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "0b");
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. } if text.contains("base-two") && text.contains("visual separators") && text.contains("10100110 represent 166 in decimal")));
    let constant = ir.symbols.iter().find(|s| s.name == "MASK").unwrap();
    assert!(constant.rust_contract.iter().flatten().any(|i| i.group == "Binary integer with separators" && i.text.contains("166 in decimal")));
}
