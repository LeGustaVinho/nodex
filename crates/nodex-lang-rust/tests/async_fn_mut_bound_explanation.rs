use nodex_lang_rust::lower_source;

#[test]
fn async_fn_mut_bound_is_explained_conditionally_on_the_function_contract() {
    let source = "fn run<F: AsyncFnMut(i32)>(f: F) {} fn where_case<F>(f: F) where F: std::ops::AsyncFnMut(i32) -> bool {} fn other<F: AsyncFn(i32)>(f: F) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["run", "where_case"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Mutable async callable bound").collect();
        assert_eq!(items.len(), 1, "{name}: {:?}", symbol.rust_contract);
        let item = items[0];
        assert!(item.text.contains("If "));
        assert!(item.text.contains("mutable access"));
        assert!(item.text.contains("future"));
        assert!(item.text.contains("borrow"));
        assert!(item.text.contains("does not itself make a call"));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].contains("AsyncFnMut(i32)"));
    }
    let other = ir.symbols.iter().find(|s| s.name == "other").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Mutable async callable bound"));
}
