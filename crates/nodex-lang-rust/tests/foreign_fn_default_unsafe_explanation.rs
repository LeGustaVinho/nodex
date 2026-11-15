use nodex_lang_rust::lower_source;
#[test]
fn foreign_function_defaults_to_unsafe_but_explicit_safe_does_not() {
    let source = "unsafe extern \"C\" { fn default_fn(); safe fn safe_api(); pub safe fn public_safe(); unsafe fn explicit_unsafe(); // safe in a comment\n #[allow(safe)] fn after_comment(); } trait T { fn ordinary(); }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["default_fn", "after_comment"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Foreign function unsafe by default").collect();
        assert_eq!(items.len(), 1, "{name}: {:?}", symbol.rust_contract);
        let item = items[0];
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "fn");
        assert!(item.text.contains("unsafe to call by default") && item.text.contains("call needs an unsafe context") && item.text.contains("explicitly `safe fn`") && item.text.contains("cannot verify"));
    }
    for name in ["safe_api", "public_safe", "explicit_unsafe", "ordinary"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Foreign function unsafe by default"), "{name}: {:?}", symbol.rust_contract);
    }
}
