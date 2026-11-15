use nodex_lang_rust::lower_source;

#[test]
fn explains_clippy_msrv_preserving_the_written_value() {
    let source = "#[clippy::msrv = \"1.70\"] #[inline] fn f() {} #[clippy::msrv = \"1.30.0\"] struct S; #[clippy::other = \"1.70\"] fn other() {} #[allow(clippy::msrv)] fn lint() {}";
    let ir = lower_source("src/lib.rs", source);
    for (name, version) in [("f", "\"1.70\""), ("S", "\"1.30.0\"")] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let detail = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Clippy minimum Rust version").unwrap();
        assert!(detail.text.contains(version) && detail.text.contains("minimum supported Rust version") && detail.text.contains("does not install or select a compiler") && detail.text.contains("accepted must be checked"));
        assert!(source[detail.span.start_byte as usize..detail.span.end_byte as usize].starts_with("#[clippy::msrv"));
    }
    for name in ["other", "lint"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Clippy minimum Rust version"));
    }
}
