use nodex_lang_rust::lower_source;

#[test]
fn explicit_rust_abi_is_explained_on_the_function_only() {
    let source = "extern \"Rust\" fn direct() {} unsafe extern \"Rust\" fn dangerous() {} extern \"C\" fn c() {} fn plain() {} type Pointer = extern \"Rust\" fn(); unsafe extern \"Rust\" { fn foreign(); } trait T { extern \"Rust\" fn required(); }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["direct", "dangerous", "required"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Explicit Rust ABI").collect();
        assert_eq!(facts.len(), 1, "{}: {:?}", name, symbol.rust_contract);
        assert_eq!(&source[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], "extern \"Rust\"");
        assert!(facts[0].text.contains("native application binary interface") && facts[0].text.contains("default") && facts[0].text.contains("no stability guarantees") && facts[0].text.contains("does not by itself export"));
    }
    for name in ["c", "plain", "Pointer", "foreign"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Explicit Rust ABI"), "{}", name);
    }
}
