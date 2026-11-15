use nodex_lang_rust::lower_source;

#[test]
fn repr_u8_explains_enum_tag_without_claiming_payload_is_one_byte() {
    let source = "#[repr(u8)] enum Status { Idle = 0, Running = 1 } #[repr(C, u8)] enum Payload { Data(u32) } #[repr(u16)] enum Other { A } #[repr(u8)] struct NotEnum;";
    let file = lower_source("src/lib.rs", source);
    for name in ["Status", "Payload"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Enum u8 representation").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert!((&source[item.span.start_byte as usize..item.span.end_byte as usize]).starts_with("#[repr("));
        assert!(item.text.contains("0 through 255"));
        assert!(item.text.contains("not necessarily one byte"));
        assert!(item.text.contains("not a complete foreign-interface safety guarantee"));
    }
    for name in ["Other", "NotEnum"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Enum u8 representation"));
    }
}
