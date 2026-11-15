use nodex_lang_rust::lower_source;

#[test]
fn win64_abi_explains_target_dependent_calling_convention() {
    let source = r#"extern "win64" fn f() {} unsafe extern "win64" fn g() {} unsafe extern "win64" { fn foreign(); } extern "C" fn c() {} extern "win64-unwind" fn unwind() {} fn ordinary() {}"#;
    let file = lower_source("src/lib.rs", source);
    for name in ["f", "g", "foreign"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Win64 calling convention").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "extern \"win64\"");
        for phrase in ["Windows x86-64 calling convention", "depends on the compilation target", "does not establish", "unmangled symbol", "foreign-language boundary"] {
            assert!(item.text.contains(phrase), "{name}: {}", item.text);
        }
        assert!(item.text.contains(if name == "foreign" { "linked code must provide" } else { "Rust function body" }));
    }
    for name in ["c", "unwind", "ordinary"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Win64 calling convention"), "{name}");
    }
}
