use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn u8_literal_suffix_is_explained_in_flow_and_constant_contract() {
    let source = "const WIDTH: u8 = 123u8; fn run() { let x = 123u8; consume(0xffu8); let y = 12_u8; let other = 123u16; let text = \"123u8\"; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "run").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Integer literal with u8 suffix")).collect();
    assert_eq!(sites.len(), 3, "{:?}", flow.reading);
    for site in sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "u8");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("unsigned 8-bit") && text.contains("0 through 255") && text.contains("not a variable name or a runtime conversion") && text.contains("overflowing_literals")));
    }
    let constant = ir.symbols.iter().find(|symbol| symbol.name == "WIDTH").unwrap();
    let item = constant.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Integer literal with u8 suffix").expect("constant suffix contract");
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "u8");
    assert!(item.text.contains("123u8") && item.text.contains("0 through 255"));
}
