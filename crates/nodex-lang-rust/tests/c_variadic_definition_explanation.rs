use nodex_lang_rust::lower_source;

#[test]
fn explains_unsafe_c_variadic_definition_without_matching_other_functions() {
    let source = "unsafe extern \"C\" fn f(x: i32, mut args: ...) {} unsafe extern \"C\" fn fixed(x: i32) {} unsafe extern \"C-unwind\" fn other(x: i32, mut args: ...) {} unsafe extern \"C\" { fn foreign(x: i32, ...); }";
    let file = lower_source("src/lib.rs", source);
    let symbol = file.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Variadic C function definition").unwrap_or_else(|| panic!("{:?}", symbol.rust_contract));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "...");
    assert!(item.text.contains("zero or more extra arguments") && item.text.contains("C argument promotions") && item.text.contains("undefined behavior") && item.text.contains("unstable `c_variadic`") && item.text.contains("callers must uphold"));
    for name in ["fixed", "other", "foreign"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Variadic C function definition"), "{name}");
    }
}
