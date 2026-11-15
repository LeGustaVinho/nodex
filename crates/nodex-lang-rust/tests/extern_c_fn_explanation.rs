use nodex_lang_rust::lower_source;

#[test]
fn explains_c_abi_on_definitions_without_treating_other_abis_as_c() {
    let source = "extern \"C\" fn f() {} unsafe extern \"C\" fn unsafe_c() {} extern \"C-unwind\" fn unwind() {} extern \"Rust\" fn rust_abi() {} fn plain() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "unsafe_c"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let detail = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "C calling convention").unwrap();
        assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "extern \"C\"");
        assert!(detail.text.contains("target platform's C application binary interface") && detail.text.contains("Rust function body") && detail.text.contains("does not export an unmangled symbol") && detail.text.contains("unsafe"));
    }
    for name in ["unwind", "rust_abi", "plain"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "C calling convention"), "{name}");
    }
}
