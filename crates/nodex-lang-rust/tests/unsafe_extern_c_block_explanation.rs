use nodex_lang_rust::lower_source;

#[test]
fn unsafe_c_block_is_explained_on_its_module_and_foreign_functions() {
    let source = "unsafe extern \"C\" { fn native(x: i32) -> i32; safe fn safe_native(); static GLOBAL: i32; } unsafe extern \"C-unwind\" { fn unwind(); } unsafe extern \"system\" { fn sys(); } extern \"C\" { fn legacy(); } mod nested { unsafe extern \"C\" { fn nested_fn(); } }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.parent.is_none()).unwrap();
    let nested = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.name == "nested").unwrap();
    for symbol in [root, nested] {
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Unsafe C ABI external block").collect();
        assert_eq!(items.len(), 1, "{}: {:?}", symbol.name, symbol.rust_contract);
        assert!(items[0].text.contains("does not define their bodies or call them") && items[0].text.contains("unsafe to call by default") && items[0].text.contains("unless explicitly declared `safe`") && items[0].text.contains("2024 edition") && items[0].text.contains("cannot establish"));
        assert!(source[items[0].span.start_byte as usize..items[0].span.end_byte as usize].starts_with("unsafe extern \"C\""));
    }
    for name in ["native", "safe_native", "nested_fn"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(symbol.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Unsafe C ABI external block"), "{}", name);
    }
    for name in ["unwind", "sys", "legacy"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Unsafe C ABI external block"), "{}", name);
    }
}
