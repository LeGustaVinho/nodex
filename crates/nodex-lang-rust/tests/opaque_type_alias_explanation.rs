use nodex_lang_rust::lower_source;

#[test]
fn opaque_alias_explains_hidden_type_and_feature_without_matching_return_position() {
    let source = "type T = impl Trait; type Plain = u32; fn f() -> impl Trait { todo!() }";
    let file = lower_source("src/lib.rs", source);
    let symbol = file.symbols.iter().find(|symbol| symbol.name == "T").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Opaque type alias").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "impl Trait");
    assert!(item.text.contains("hidden") && item.text.contains("trait bounds") && item.text.contains("defining uses") && item.text.contains("type_alias_impl_trait") && item.text.contains("does not by itself require heap allocation"));
    for name in ["Plain", "f"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Opaque type alias"));
    }
}
