use nodex_lang_rust::lower_source;

#[test]
fn explicit_unsafe_foreign_function_explains_c_abi_and_caller_obligations() {
    let source = "unsafe extern \"C\" { unsafe fn f(); fn implicit(); } unsafe extern \"system\" { unsafe fn platform(); } unsafe fn ordinary() {}";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Explicitly unsafe foreign function").unwrap_or_else(|| panic!("{:?}", symbol.rust_contract));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "unsafe");
    assert!(item.text.contains("C calling convention") && item.text.contains("does not provide the body"));
    assert!(item.text.contains("unsafe context"));
    assert!(item.text.contains("caller must uphold") && item.text.contains("does not itself verify"));
    for name in ["implicit", "platform", "ordinary"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Explicitly unsafe foreign function"));
    }
}
