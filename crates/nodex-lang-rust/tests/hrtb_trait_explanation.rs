use nodex_lang_rust::lower_source;

#[test]
fn higher_ranked_trait_bounds_are_explained_on_their_owning_signature() {
    let source = "trait Custom {} fn generic<F: for<'a> Fn(&'a i32)>(f: F) {} fn where_bound<F>() where F: for<'b> Fn(&'b i32) {} fn simple<F: Fn(&i32)>(f: F) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["generic", "where_bound"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let items = symbol.rust_contract.as_ref().unwrap();
        let item = items.iter().find(|item| item.group == "Higher-ranked trait bound").expect("higher-ranked bound visible");
        assert!(item.text.contains("every applicable choice"));
        assert!(item.text.contains("does not require the borrow to be 'static"));
        assert!(item.text.contains("does not prove which trait"));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("for<"));
    }
    let simple = ir.symbols.iter().find(|s| s.name == "simple").unwrap();
    assert!(!simple.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Higher-ranked trait bound"));
}
