use nodex_lang_rust::lower_source;

#[test]
fn explains_target_abi_and_unwinding_without_promising_panic_behavior() {
    let source = "extern \"system-unwind\" fn f() {} pub unsafe extern \"system-unwind\" fn g() {} extern \"system\" fn h() {} extern \"C-unwind\" fn c() {} fn plain() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "g"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "System calling convention with unwinding").unwrap();
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "extern \"system-unwind\"");
        for expected in ["target-dependent", "same calling convention as `system`", "permits unwinding", "panic=abort", "automatically safe", "library import"] {
            assert!(item.text.contains(expected), "missing {expected}");
        }
    }
    for name in ["h", "c", "plain"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "System calling convention with unwinding"));
    }
}
