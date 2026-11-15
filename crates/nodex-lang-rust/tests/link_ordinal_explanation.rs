use nodex_lang_rust::lower_source;

#[test]
fn foreign_ordinal_import_explains_target_and_restrictions() {
    let source = "#[link(name = \"demo\", kind = \"raw-dylib\")] unsafe extern \"system\" { #[link_ordinal(1)] fn by_ordinal(x: i32) -> i32; #[link_name = \"other\"] fn by_name(); } #[link_ordinal(7)] fn outside() {}";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|s| s.name == "by_ordinal").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Foreign import by ordinal").collect();
    assert_eq!(items.len(), 1, "{:?}", symbol.rust_contract);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "#[link_ordinal(1)]");
    assert!(items[0].text.contains("by_ordinal") && items[0].text.contains("ordinal(1)") && items[0].text.contains("raw-dylib") && items[0].text.contains("Windows") && items[0].text.contains("link_name") && items[0].text.contains("does not prove"));
    for name in ["by_name", "outside"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Foreign import by ordinal"));
    }
}
