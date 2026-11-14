use nodex_lang_rust::lower_source;
#[test]
fn explains_shared_callable_bounds_in_inline_and_where_forms() {
    let source = "fn inline<F: Fn(i32) -> i32>(f: F) -> i32 { f(1) } fn via_where<F>(f: F) where F: Fn(i32) -> i32 {} fn mutable<F: FnMut(i32) -> i32>(f: F) {} fn pointer(f: fn(i32) -> i32) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["inline", "via_where"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Shared callable bound").unwrap();
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "Fn(i32) -> i32");
        assert!(item.text.contains("If Fn resolves") && item.text.contains("shared access") && item.text.contains("interior mutability") && item.text.contains("does not establish"));
    }
    for name in ["mutable", "pointer"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Shared callable bound"));
    }
}
