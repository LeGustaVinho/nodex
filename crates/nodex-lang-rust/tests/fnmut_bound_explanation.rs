use nodex_lang_rust::lower_source;

#[test]
fn explains_fnmut_in_inline_and_where_bounds_without_confusing_fn() {
    let source = "fn a<F: FnMut(i32)>(f: F) {} fn b<F>(f: F) where F: FnMut(i32) -> bool {} fn c<F: Fn(i32)>(f: F) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["a", "b"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Mutable callable bound").expect("FnMut contract");
        assert!(item.text.contains("mutable access"));
        assert!(item.text.contains("may change captured data"));
        assert!(item.text.contains("does not prove"));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("FnMut(i32)"));
    }
    let c = ir.symbols.iter().find(|symbol| symbol.name == "c").unwrap();
    assert!(!c.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Mutable callable bound"));
}
