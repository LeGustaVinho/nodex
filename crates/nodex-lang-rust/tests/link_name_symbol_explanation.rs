use nodex_lang_rust::lower_source;

#[test]
fn foreign_link_name_is_preserved_on_the_declared_function() {
    let source = r##"#[link(name = "m")] unsafe extern "C" { #[link_name = "external_symbol"] #[allow(dead_code)] fn local(); #[link_name = r#"other_symbol"#] fn raw(); fn plain(); } #[link_name = "not_foreign"] fn ordinary() {}"##;
    let ir = lower_source("src/lib.rs", source);
    for (name, written) in [("local", "\"external_symbol\""), ("raw", "r#\"other_symbol\"#")] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Foreign linker symbol name").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert!(item.text.contains(written) && item.text.contains(name));
        assert!(item.text.contains("not a new Rust function or a library name") && item.text.contains("does not verify") && item.text.contains("nonempty"));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("#[link_name ="));
    }
    for name in ["plain", "ordinary"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Foreign linker symbol name"), "{name}");
    }
}
