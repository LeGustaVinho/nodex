use nodex_lang_rust::lower_source;

#[test]
fn link_name_on_foreign_block_is_visible_on_its_function_symbols() {
    let source = "#[link(name = \"m\")] unsafe extern \"C\" { fn cos(x: f64) -> f64; fn sin(x: f64) -> f64; } unsafe extern \"C\" { fn other(); } fn ordinary() {}";
    let file = lower_source("src/lib.rs", source);
    for name in ["cos", "sin"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Native library link").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[link(name = \"m\")]");
        assert!(item.text.contains("\"m\""));
        assert!(item.text.contains("target, toolchain"));
        assert!(item.text.contains("does not guarantee"));
        assert!(item.text.contains("calling convention"));
    }
    for name in ["other", "ordinary"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Native library link"));
    }
}
