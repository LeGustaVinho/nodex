use nodex_lang_rust::lower_source;

#[test]
fn higher_ranked_where_borrow_is_universally_quantified() {
    let source = "trait Trait {} fn f<T>() where for<'a> &'a T: Trait {} fn ordinary<T>() where T: Trait {} fn different<F>() where F: for<'a> Fn(&'a str) {}";
    let file = lower_source("src/lib.rs", source);
    let symbol = file.symbols.iter().find(|s| s.name == "f").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Higher-ranked borrowed type in where clause").collect();
    assert_eq!(items.len(), 1, "{:?}", symbol.rust_contract);
    assert!(!symbol.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Higher-ranked trait bound"));
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "for<'a> &'a T: Trait");
    for fragment in ["every applicable choice", "&'a T must satisfy Trait", "not automatically on the underlying type", "does not create or extend a borrow", "prove which trait"] { assert!(items[0].text.contains(fragment), "{}", items[0].text); }
    for name in ["ordinary", "different"] {
        let symbol = file.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Higher-ranked borrowed type in where clause"));
    }
}
