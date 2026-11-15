use nodex_lang_rust::lower_source;
#[test]
fn empty_tuple_struct_has_own_type_no_fields_and_zero_argument_constructor() {
    let source = "struct EmptyTuple(); struct NotEmpty(i32); struct Unit; struct Record {} fn f() { let _ = EmptyTuple(); }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|s| s.name == "EmptyTuple").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Empty tuple struct").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "struct EmptyTuple();");
    assert!(item.text.contains("zero fields") && item.text.contains("EmptyTuple()") && item.text.contains("zero-sized") && item.text.contains("no `.0` field") && item.text.contains("not the unit type"));
    assert!(symbol.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Tuple struct"));
    for name in ["NotEmpty", "Unit", "Record"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Empty tuple struct"), "{name}");
    }
}
