#[test]
fn enum_contract_explains_variant_data_and_exclusivity() {
    let source = "enum Message { Quit, Move(i32, i32) }";
    let file = nodex_lang_rust::lower_source("src/lib.rs", source);
    let symbol = file.symbols.iter().find(|s| s.name == "Message").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter()
        .find(|item| item.group == "Enum alternatives").unwrap();
    assert!(item.text.contains("exactly one variant"));
    assert!(item.text.contains("Quit carries no fields"));
    assert!(item.text.contains("Move carries positional fields of types i32, i32"));
    assert!(item.text.contains("match"));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], source);
}
