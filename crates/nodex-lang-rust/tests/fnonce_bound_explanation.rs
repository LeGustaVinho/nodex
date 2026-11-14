use nodex_lang_rust::lower_source;

#[test]
fn fnonce_bound_explains_by_value_call_without_assuming_resolution() {
    let source = "fn run<F: FnOnce(i32)>(f: F) {} fn other<F: Fn(i32), G: FnMut(i32)>(f: F, g: G) {} fn via_where<F>(f: F) where F: FnOnce(i32) -> i32 {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["run", "via_where"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "FnOnce callable bound").unwrap();
        assert!(item.text.contains("If FnOnce resolves"));
        assert!(item.text.contains("takes the callable itself by value"));
        assert!(item.text.contains("may consume captured data"));
        assert!(item.text.contains("does not mean every call consumes"));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("FnOnce(i32)"));
    }
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "other").unwrap();
    assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "FnOnce callable bound"));
}
