use nodex_lang_rust::lower_source;

#[test]
fn default_type_parameters_are_explained_on_the_type_symbol() {
    let source = "struct S<T = u32>(T); struct Pair<A, B = Vec<A>>(A, B); struct Plain<T>(T); struct Const<const N: usize = 8>;";
    let ir = lower_source("src/lib.rs", source);
    for (name, expected) in [("S", "T = u32"), ("Pair", "B = Vec<A>")] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Default generic type").unwrap();
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], expected);
        assert!(item.text.contains("omitting this argument"));
        assert!(item.text.contains("different type explicitly"));
        assert!(item.text.contains("inference"));
    }
    for name in ["Plain", "Const"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Default generic type"));
    }
}
