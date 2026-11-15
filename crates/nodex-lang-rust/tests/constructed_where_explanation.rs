use nodex_lang_rust::lower_source;
#[test]
fn where_bound_targets_constructed_type_not_directly_its_argument() {
    let source = "fn f<T>() where Vec<T>: Clone {} trait X { fn g<U>() where Option<U>: Custom; } fn plain<T>() where T: Clone {}";
    let ir = lower_source("src/lib.rs", source);
    for (name, predicate) in [("f", "Vec<T>: Clone"), ("g", "Option<U>: Custom")] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Constructed type requirement").collect();
        assert_eq!(items.len(), 1, "{name}: {:?}", symbol.rust_contract);
        let item = items[0];
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], predicate);
        assert!(item.text.contains("whole type") && item.text.contains("not directly on each type argument") && item.text.contains("depends on the resolved") && item.text.contains("neither calls `clone` nor creates a copy"));
        assert!(symbol.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Where-clause type requirement"));
    }
    let plain = ir.symbols.iter().find(|s| s.name == "plain").unwrap();
    assert!(!plain.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Constructed type requirement"));
}
