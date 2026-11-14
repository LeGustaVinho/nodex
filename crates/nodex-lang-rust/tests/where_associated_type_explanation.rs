use nodex_lang_rust::lower_source;

#[test]
fn explains_projection_bound_on_associated_type() {
    let source = "trait Source { type Item; } fn inspect<T: Source>(x: T) where T::Item: Clone {} fn other<T: Clone>(x: T) {}";
    let ir = lower_source("src/lib.rs", source);
    let inspect = ir.symbols.iter().find(|symbol| symbol.name == "inspect").unwrap();
    let item = inspect.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Associated type requirement").unwrap();
    assert!(item.text.contains("T::Item"));
    assert!(item.text.contains("rather than requiring T itself"));
    assert!(item.text.contains("If Clone denotes"));
    assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].contains("T::Item: Clone"));
    let other = ir.symbols.iter().find(|symbol| symbol.name == "other").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Associated type requirement"));
}
