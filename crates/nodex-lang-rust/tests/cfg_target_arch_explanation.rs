use nodex_lang_rust::lower_source;

#[test]
fn explains_direct_target_arch_condition_without_guessing_current_target() {
    let source = "#[cfg(target_arch = \"x86_64\")] #[inline] fn f() {} #[cfg(target_arch = \"aarch64\")] fn other() {} #[cfg(any(target_arch = \"x86_64\"))] fn nested() {}";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Target architecture condition").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[cfg(target_arch = \"x86_64\")]");
    assert!(item.text.contains("compilation target") && item.text.contains("excludes that item") && item.text.contains("not a runtime check") && item.text.contains("does not tell us"));
    for name in ["other", "nested"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Target architecture condition"));
    }
}
