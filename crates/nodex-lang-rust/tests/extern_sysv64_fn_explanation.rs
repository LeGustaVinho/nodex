use nodex_lang_rust::lower_source;

#[test]
fn sysv64_abi_explains_calling_convention_and_target() {
    let source = r#"extern "sysv64" fn f() {} unsafe extern "sysv64" fn g() {} unsafe extern "sysv64" { fn foreign(); } extern "C" fn c() {} extern "sysv64-unwind" fn unwind() {} fn ordinary() {}"#;
    let file = lower_source("src/lib.rs", source);
    for name in ["f", "g", "foreign"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "System V calling convention").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "extern \"sysv64\"");
        for phrase in ["System V x86-64 calling convention", "only on x86-64 compilation targets", "non-Windows", "does not establish the target", "unmangled symbol", "foreign-language boundary"] {
            assert!(item.text.contains(phrase), "{name}: {}", item.text);
        }
        assert!(item.text.contains(if name == "foreign" { "linked code must provide" } else { "Rust function body" }));
    }
    for name in ["c", "unwind", "ordinary"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "System V calling convention"), "{name}");
    }
}
