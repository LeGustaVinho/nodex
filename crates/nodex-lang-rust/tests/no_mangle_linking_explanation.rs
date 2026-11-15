use nodex_lang_rust::lower_source;

#[test]
fn unmangled_symbol_explanation_describes_linking_and_raw_identifiers() {
    let source = "#[unsafe(no_mangle)] pub extern \"C\" fn exported() {} #[unsafe(no_mangle)] pub fn r#type() {} #[unsafe(export_name = \"named\")] pub fn other() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["exported", "r#type"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Unmangled symbol and linking").unwrap_or_else(|| panic!("{:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[unsafe(no_mangle)]");
        assert!(item.text.contains("linker uses symbol names") && item.text.contains("calling convention") && item.text.contains("does not") && item.text.contains("prove"));
        if name == "r#type" { assert!(item.text.contains("symbol name type") && item.text.contains("identifier's name is type")); }
    }
    let other = ir.symbols.iter().find(|symbol| symbol.name == "other").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Unmangled symbol and linking"));
}
