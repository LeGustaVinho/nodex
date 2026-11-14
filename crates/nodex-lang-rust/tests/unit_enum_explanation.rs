use nodex_lang_rust::lower_source;

#[test]
fn explains_enum_with_fieldless_variants_without_claiming_data_variants_are_fieldless() {
    let source = "enum State { Idle, Running } enum Payload { Empty, Number(i32) } struct Other;";
    let ir = lower_source("src/lib.rs", source);
    let state = ir.symbols.iter().find(|symbol| symbol.name == "State").unwrap();
    let detail = state.rust_contract.as_ref().unwrap().iter()
        .find(|item| item.group == "Enumeration").unwrap();
    for part in ["Idle", "Running", "exactly one", "no additional fields", "State::Idle"] {
        assert!(detail.text.contains(part), "missing {part:?}: {}", detail.text);
    }
    for name in ["Payload", "Other"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Enumeration")));
    }
}
