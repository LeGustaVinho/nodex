use nodex_lang_rust::lower_source;

#[test]
fn combined_c_u8_enum_layout_explains_tag_and_rejects_unsupported_conclusions() {
    let source = "#[repr(C, u8)] enum Data { A(u32), B { x: u8 }, C } #[repr(u8, C,)] enum Reverse { Item(i32) } #[repr(C,u8)] enum Fieldless { A, B } #[repr(C,u8)] enum Empty {} #[repr(C)] enum COnly { A(i32) } #[repr(u8)] enum U8Only { A(i32) }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["Data", "Reverse", "Fieldless", "Empty"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "C layout with u8 tag").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert!((&source[item.span.start_byte as usize..item.span.end_byte as usize]).starts_with("#[repr("));
        assert!(item.text.contains("not necessarily the entire enum"));
        assert!(item.text.contains("does not prove"));
        if name == "Fieldless" { assert!(item.text.contains("conflicting representation hints")); }
        else if name == "Empty" { assert!(item.text.contains("zero-variant enum cannot use")); }
        else { assert!(item.text.contains("union of the variant payloads")); assert!(item.text.contains("size and alignment are one byte")); assert!(item.text.contains("not generally the same layout as repr(u8)")); }
    }
    for name in ["COnly", "U8Only"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "C layout with u8 tag"), "{name}: {:?}", symbol.rust_contract);
    }
}
