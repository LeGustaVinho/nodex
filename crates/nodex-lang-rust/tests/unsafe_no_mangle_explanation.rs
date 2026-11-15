use nodex_lang_rust::lower_source;

#[test]
fn unsafe_no_mangle_explains_symbol_and_unverified_safety() {
    let source = "#[unsafe(no_mangle)] pub extern \"C\" fn exported() {} #[unsafe(no_mangle)] pub static VALUE: i32 = 1; #[allow(dead_code)] fn ordinary() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["exported", "VALUE"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Unsafe unmangled symbol").unwrap_or_else(|| panic!("{:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[unsafe(no_mangle)]");
        assert!(item.text.contains("without Rust's usual name mangling") && item.text.contains(name));
        assert!(item.text.contains("does not collide") && item.text.contains("cannot prove") && item.text.contains("calling convention"));
    }
    let ordinary = ir.symbols.iter().find(|symbol| symbol.name == "ordinary").unwrap();
    assert!(!ordinary.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Unsafe unmangled symbol"));
}
