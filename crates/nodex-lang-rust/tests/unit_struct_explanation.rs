use nodex_lang_rust::lower_source;

#[test]
fn explains_unit_struct_but_not_structs_with_field_lists() {
    let ir = lower_source("src/lib.rs", "struct Marker; struct Record {} struct Tuple();");
    let marker = ir.symbols.iter().find(|symbol| symbol.name == "Marker").unwrap();
    let item = marker.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Unit struct").unwrap();
    assert!(item.text.contains("distinct struct type with no fields"));
    assert!(item.text.contains("Writing Marker as a value constructs an instance"));
    for name in ["Record", "Tuple"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Unit struct")));
    }
}
