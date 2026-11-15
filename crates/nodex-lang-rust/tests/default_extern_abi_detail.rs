use nodex_lang_rust::lower_source;
#[test]
fn omitted_extern_abi_defaults_to_c_only_for_actual_definitions() {
    let source = "extern fn f() {} pub unsafe extern fn g() {} extern \"C\" fn explicit() {} extern \"Rust\" fn rust_abi() {} fn normal() {} unsafe extern { fn imported(); }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "g"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Default extern calling convention").collect();
        assert_eq!(items.len(), 1, "{name}: {:?}", symbol.rust_contract);
        let item = items[0];
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "extern");
        assert!(item.text.contains("defaults to the target's") && item.text.contains("extern \"C\" fn") && item.text.contains("ordinary function without `extern`") && item.text.contains("neither makes the function unsafe") && item.text.contains("not a foreign declaration"));
    }
    for name in ["explicit", "rust_abi", "normal", "imported"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Default extern calling convention"), "{name}");
    }
}
