use nodex_lang_rust::lower_source;

#[test]
fn inline_never_is_explained_as_a_hint_on_functions_with_bodies() {
    let source = "#[inline( never )] #[cold] fn slow() {} #[inline(never)] async fn later() {} trait T { #[inline(never)] fn default(&self) {} #[inline(never)] fn required(&self); } #[inline] fn plain() {} #[inline(always)] fn always() {} #[cfg_attr(feature = \"x\", inline(never))] fn conditional() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["slow", "later", "default"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Inline never hint").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("#[inline("));
        assert!(item.text.contains("compiler may ignore") && item.text.contains("does not stop") && item.text.contains("does not guarantee") && item.text.contains("generated `poll`") && item.text.contains("default body"));
    }
    for name in ["required", "plain", "always", "conditional"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Inline never hint"), "{name}");
    }
}
