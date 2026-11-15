use nodex_lang_rust::lower_source;

#[test]
fn pair_alias_reuses_one_generic_type_in_both_tuple_positions() {
    let source = "type Pair<T> = (T, T); type One<T> = (T,); type Mixed<T, U> = (T, U); type Wrapped<T> = (T, Option<T>);";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "Pair").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Generic pair type alias").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "type Pair<T> = (T, T);");
    for phrase in ["same type occupies both positions", "values need not be equal", "`Pair<i32>`", "`(i32, i32)`", "`.0` and `.1`", "not a distinct wrapper type"] { assert!(item.text.contains(phrase), "{}", item.text); }
    for name in ["One", "Mixed", "Wrapped"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Generic pair type alias"));
    }
}
