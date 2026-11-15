use nodex_lang_rust::lower_source;

#[test]
fn raw_abi_string_explains_c_convention_and_raw_delimiters() {
    let source = "extern r\"C\" fn f() {} extern \"C\" fn ordinary() {}";
    let file = lower_source("src/lib.rs", source);
    let symbol = file.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Raw string ABI on a function").unwrap_or_else(|| panic!("{:?}", symbol.rust_contract));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "r\"C\"");
    assert!(item.text.contains("raw string literal") && item.text.contains("C calling convention") && item.text.contains("does not export an unmangled symbol") && item.text.contains("unsafe"));
    let ordinary = file.symbols.iter().find(|symbol| symbol.name == "ordinary").unwrap();
    assert!(!ordinary.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Raw string ABI on a function"));
}
