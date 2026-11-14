use nodex_lang_rust::lower_source;

#[test]
fn const_generic_value_is_explained_on_functions_and_trait_methods() {
    let text = "fn size<const N: usize>() -> usize { N } trait T { fn f<const M: usize>(); } fn ordinary<T>() {}";
    let ir = lower_source("src/lib.rs", text);
    for (name, written) in [("size", "const N: usize"), ("f", "const M: usize")] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|i| i.group == "Const generic parameter").unwrap();
        assert_eq!(&text[item.span.start_byte as usize..item.span.end_byte as usize], written);
        assert!(item.text.contains("compile-time value"));
        assert!(item.text.contains("ordinary runtime argument"));
        assert!(item.text.contains("inference"));
    }
    let ordinary = ir.symbols.iter().find(|s| s.name == "ordinary").unwrap();
    assert!(!ordinary.rust_contract.as_ref().into_iter().flatten().any(|i| i.group == "Const generic parameter"));
}
