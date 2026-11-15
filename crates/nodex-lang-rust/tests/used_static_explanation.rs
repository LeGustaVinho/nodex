use nodex_lang_rust::lower_source;

#[test]
fn used_explains_object_retention_without_promising_final_link_retention() {
    let source = "#[used] #[allow(dead_code)] static KEEP: u8 = 1; static NORMAL: u8 = 2; #[used(compiler)] static OTHER: u8 = 3; #[used] const NOT_STATIC: u8 = 4;";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "KEEP").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Retain static in object output").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[used]");
    assert!(item.text.contains("object file") && item.text.contains("linker may still remove") && item.text.contains("does not itself export"));
    for name in ["NORMAL", "OTHER", "NOT_STATIC"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Retain static in object output"), "{name}");
    }
}
