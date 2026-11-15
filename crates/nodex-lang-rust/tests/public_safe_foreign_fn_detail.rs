use nodex_lang_rust::lower_source;
#[test]
fn public_safe_foreign_function_explains_visibility_and_safety_separately() {
    let source = "unsafe extern \"C\" { pub safe fn f(); safe fn private(); pub(crate) safe fn restricted(); pub unsafe fn risky(); pub fn implicit(); } pub fn ordinary() {}";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|s| s.name == "f").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Public safe foreign function").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "pub");
    assert!(item.text.contains("accessible path") && item.text.contains("safe Rust can call") && item.text.contains("signature, ABI and safety promise") && item.text.contains("do not prove"));
    assert!(symbol.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Explicitly safe foreign function"));
    for name in ["private", "restricted", "risky", "implicit", "ordinary"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Public safe foreign function"), "{name}");
    }
}
