use nodex_lang_rust::lower_source;

#[test]
fn custom_foreign_abi_is_explained_without_claiming_support() {
    let source = "unsafe extern \"custom\" { fn first(); safe fn second(x: i32); } extern \"C\" { fn c_api(); } extern \"system\" { fn system_api(); } fn ordinary() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["first", "second"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Named foreign ABI").collect();
        assert_eq!(items.len(), 1, "{name}: {:?}", symbol.rust_contract);
        let item = items[0];
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "\"custom\"");
        assert!(item.text.contains("must recognize and support") && item.text.contains("not automatically a valid") && item.text.contains("rejected") && item.text.contains("does not define or call") && item.text.contains("does not establish their safety"));
    }
    for name in ["c_api", "system_api", "ordinary"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Named foreign ABI"), "{name}");
    }
}
