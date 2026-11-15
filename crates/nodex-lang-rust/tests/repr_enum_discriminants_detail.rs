use nodex_lang_rust::lower_source;

#[test]
fn repr_u8_and_discriminants_explain_unit_and_payload_variants_together() {
    let source = "#[repr(u8)] enum E { A = 1, B(u8) = 2 }";
    let file = lower_source("src/lib.rs", source);
    let en = file.symbols.iter().find(|symbol| symbol.name == "E").unwrap();
    let items = en.rust_contract.as_ref().unwrap();
    let repr = items.iter().find(|item| item.group == "Enum u8 representation").unwrap();
    assert!(repr.text.contains("tag") && repr.text.contains("not necessarily one byte"));
    let variants: Vec<_> = items.iter().filter(|item| item.group == "Enum discriminant").collect();
    assert_eq!(variants.len(), 2);
    assert!(variants.iter().any(|item| item.text.contains("`= 1` after variant A")));
    assert!(variants.iter().any(|item| item.text.contains("`= 2` after variant B")));
    assert!(variants.iter().all(|item| item.text.contains("not a field holding extra data")));
}
