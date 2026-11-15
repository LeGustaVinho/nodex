use nodex_lang_rust::lower_source;

#[test]
fn automatically_derived_is_explained_on_owning_type_members_and_empty_impl() {
    let source = "struct S; #[automatically_derived] impl Foo for S { fn run(&self) {} } impl Other for S { fn skip(&self) {} } struct Empty; #[automatically_derived] impl Foo for Empty {} #[automatically_derived] fn invalid_position() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["S", "run", "Empty"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Automatically derived implementation").collect();
        assert_eq!(items.len(), 1, "{name}: {:?}", symbol.rust_contract);
        assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "#[automatically_derived]");
        assert!(items[0].text.contains("no direct effect"));
        assert!(items[0].text.contains("does not prove"));
        assert!(items[0].text.contains("not to the whole type"));
    }
    for name in ["skip", "invalid_position"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Automatically derived implementation"), "{name}: {:?}", symbol.rust_contract);
    }
}
