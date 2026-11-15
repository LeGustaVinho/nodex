use nodex_lang_rust::lower_source;

#[test]
fn unsafe_link_section_preserves_written_name_and_explains_safety() {
    let source = r##"#[unsafe(link_section = ".my_section")] static X: u8 = 0; #[unsafe(link_section = ".my section")] fn f() {} #[unsafe(link_section = r#".raw section"#)] static Y: u8 = 0; #[unsafe(export_name = ".my_section")] fn other() {} #[unsafe(no_mangle)] fn plain() {} #[link_section = ".my_section"] static LEGACY: u8 = 0; #[cfg_attr(feature = "x", unsafe(link_section = ".my_section"))] static CONDITIONAL: u8 = 0;"##;
    let ir = lower_source("src/lib.rs", source);
    for (name, written) in [("X", "\".my_section\""), ("f", "\".my section\""), ("Y", "r#\".raw section\"#")] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Unsafe object-file section").collect();
        assert_eq!(facts.len(), 1, "{name}: {:?}", symbol.rust_contract);
        let item = facts[0];
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], written);
        assert!(item.text.contains(written));
        assert!(item.text.contains("object-file section"));
        assert!(item.text.contains("mutable data in a read-only section"));
        assert!(item.text.contains("does not prove that the placement is safe"));
        assert!(item.text.contains("only the first takes effect"));
        assert!(item.text.contains("edition 2024 requires"));
    }
    for name in ["other", "plain", "LEGACY", "CONDITIONAL"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Unsafe object-file section"), "{name}: {:?}", symbol.rust_contract);
    }
}
