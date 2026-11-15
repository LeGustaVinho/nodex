use nodex_lang_rust::lower_source;

#[test]
fn c_variadic_declaration_explains_extra_arguments_and_safety() {
    let source = "unsafe extern \"C\" { fn f(x: i32, ...); safe fn ignore(x: i32, ...); fn fixed(x: i32); } unsafe extern \"system\" { fn other(x: i32, ...); }";
    let file = lower_source("src/lib.rs", source);
    for name in ["f", "ignore"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Variadic foreign C function").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "...");
        assert!(item.text.contains("zero or more additional arguments"));
        assert!(item.text.contains("argument promotions"));
        assert!(item.text.contains("undefined behavior"));
        assert!(item.text.contains("unsafe to call by default"));
        assert!(item.text.contains("explicitly safe declaration"));
    }
    for name in ["fixed", "other"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Variadic foreign C function"));
    }
}
