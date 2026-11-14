use nodex_lang_rust::lower_source;

#[test]
fn where_clause_explains_all_predicates_on_function_and_impl_member() {
    let source = "fn f<T, U>() where T: A, U: B {} struct S<T>(T); impl<T> S<T> where T: A { fn g(&self) {} } fn plain() {}";
    let ir = lower_source("src/lib.rs", source);
    let f = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let detail = f.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Where clause").unwrap();
    assert!(detail.text.contains("all requirements must hold"));
    assert!(detail.text.contains("T: A"));
    assert!(detail.text.contains("U: B"));
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "where T: A, U: B");
    let g = ir.symbols.iter().find(|symbol| symbol.name == "g").unwrap();
    assert!(g.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Where clause" && item.text.contains("T: A")));
    let plain = ir.symbols.iter().find(|symbol| symbol.name == "plain").unwrap();
    assert!(!plain.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Where clause"));
}
