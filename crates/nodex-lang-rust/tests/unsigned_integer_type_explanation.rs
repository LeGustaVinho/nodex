use nodex_lang_rust::lower_source;
use nodex_ir::reading::ReadingContent;

#[test]
fn unsigned_types_are_explained_in_visible_declarations() {
    let source = "struct S { byte: u8, count: u64 } type Alias = Option<u16>; fn f(x: u32) -> u8 { let ignored: Option<u64> = None; x as u8 }";
    let ir = lower_source("src/lib.rs", source);
    let item = |symbol: &str, spelling: &str| {
        let symbol = ir.symbols.iter().find(|s| s.name == symbol).unwrap();
        symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Unsigned integer type" && &source[item.span.start_byte as usize..item.span.end_byte as usize] == spelling).unwrap().text.clone()
    };
    assert!(item("S", "u8").contains("0 through 255"));
    assert!(item("S", "u64").contains("18,446,744,073,709,551,615"));
    assert!(item("Alias", "u16").contains("65,535"));
    assert!(item("f", "u32").contains("4,294,967,295"));
    assert!(item("f", "u8").contains("8-bit"));
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. }
        if label == "Unsigned integer type" && text.contains("18,446,744,073,709,551,615"))));
    let function = ir.symbols.iter().find(|s| s.name == "f").unwrap();
    assert!(!function.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Unsigned integer type" && &source[item.span.start_byte as usize..item.span.end_byte as usize] == "u64"));
}
