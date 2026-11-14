use nodex_lang_rust::lower_source;

#[test]
fn async_fn_bound_is_explained_without_assuming_trait_identity() {
    let source = "fn inline<F: AsyncFn(i32)>(f: F) {} fn via_where<F>(f: F) where F: AsyncFn(i32) -> i32 {} fn ordinary<F: Fn(i32)>(f: F) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["inline", "via_where"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Async callable bound").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert!(item.text.contains("future") && item.text.contains("polled or awaited") && item.text.contains("does not prove") && item.text.contains("shared access"));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("AsyncFn(i32)"));
    }
    let ordinary = ir.symbols.iter().find(|symbol| symbol.name == "ordinary").unwrap();
    assert!(!ordinary.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Async callable bound"));
}
