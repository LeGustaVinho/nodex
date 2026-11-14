use nodex_lang_rust::lower_source;

#[test]
fn const_function_explains_context_and_runtime_calls() {
    let source = "const fn twice(x: u32) -> u32 { x * 2 } fn ordinary(x: u32) -> u32 { x * 2 } const N: u32 = 1; impl S { const fn create() -> Self { S } } struct S;";
    let file = lower_source("src/lib.rs", source);
    for name in ["twice", "create"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Const function").unwrap();
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "const");
        assert!(item.text.contains("constant context"));
        assert!(item.text.contains("may execute at runtime"));
        assert!(item.text.contains("does not guarantee"));
        assert!(item.text.contains("restrictions"));
    }
    for name in ["ordinary", "N"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Const function"));
    }
}
