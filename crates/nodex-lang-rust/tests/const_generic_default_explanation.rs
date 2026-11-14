use nodex_lang_rust::lower_source;

#[test]
fn explains_default_const_argument_on_the_struct_symbol() {
    let text = "struct S<const N: usize = 8>; struct Other<const N: usize>; struct TypeDefault<T = u32>(T);";
    let ir = lower_source("src/lib.rs", text);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "S").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Default const generic").unwrap();
    assert_eq!(&text[item.span.start_byte as usize..item.span.end_byte as usize], "const N: usize = 8");
    assert!(item.text.contains("compile-time value"));
    assert!(item.text.contains("default 8"));
    assert!(item.text.contains("explicit const argument"));
    assert!(item.text.contains("different types"));
    for name in ["Other", "TypeDefault"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Default const generic"));
    }
}
