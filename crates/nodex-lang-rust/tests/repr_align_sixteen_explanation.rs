use nodex_lang_rust::lower_source;

#[test]
fn explains_repr_align_16_without_claiming_fixed_field_order() {
    let source = "#[repr(align(16))] struct Aligned(u8); #[repr(align( 16 ))] enum E { A } #[repr(align(8))] struct Other(u8); #[repr(C)] struct C(u8);";
    let ir = lower_source("src/lib.rs", source);
    for name in ["Aligned", "E"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Minimum alignment").unwrap();
        assert!(item.text.contains("at least 16 bytes") && item.text.contains("can be larger") && item.text.contains("padding") && item.text.contains("does not by itself give the type C layout"));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("#[repr(align("));
    }
    for name in ["Other", "C"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Minimum alignment"));
    }
}
