use nodex_lang_rust::lower_source;

#[test]
fn explains_c_unwind_definitions_and_distinguishes_other_abis() {
    let source = "extern \"C-unwind\" fn f() {} unsafe extern \"C-unwind\" fn unsafe_f() {} extern \"C\" fn c() {} extern \"Rust\" fn rust() {} fn plain() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "unsafe_f"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let detail = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "C-unwind calling convention").unwrap();
        assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "extern \"C-unwind\"");
        assert!(detail.text.contains("permits unwinding across") && detail.text.contains("configured to abort") && detail.text.contains("does not export an unmangled symbol") && detail.text.contains("unsafe"));
    }
    for name in ["c", "rust", "plain"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "C-unwind calling convention"), "{name}");
    }
}
