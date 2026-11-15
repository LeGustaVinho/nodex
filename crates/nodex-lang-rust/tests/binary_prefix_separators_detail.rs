use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn binary_prefix_underscores_are_explained_in_flow_and_constant() {
    let source = "const N: u8 = 0b________1; fn f() { let n = 0b________1; let s = \"0b________1\"; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Binary integer with separators")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    assert!(matches!(&facts[0].content, ReadingContent::Detail { text, .. } if text.contains("0b________1") && text.contains("visual separators") && text.contains("1 represent 1 in decimal")));
    let constant = ir.symbols.iter().find(|symbol| symbol.name == "N").unwrap();
    assert!(constant.rust_contract.iter().flatten().any(|item| item.group == "Binary integer with separators" && item.text.contains("1 represent 1 in decimal")));
}
