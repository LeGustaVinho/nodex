use nodex_lang_rust::lower_source;

#[test]
fn explicit_sized_bound_is_explained_without_confusing_relaxation() {
    let text = "fn explicit<T: Sized>() {} fn qualified<T>() where T: core::marker::Sized {} fn relaxed<T: ?Sized>() {} fn other<T: Send>() {}";
    let ir = lower_source("src/lib.rs", text);
    for name in ["explicit", "qualified"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let details: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Sized type requirement").collect();
        assert_eq!(details.len(), 1, "{name}: {details:?}");
        assert!(details[0].text.contains("size is known at compile time"));
        assert!(details[0].text.contains("spelling alone does not prove"));
        assert!(text[details[0].span.start_byte as usize..details[0].span.end_byte as usize].ends_with("Sized"));
    }
    for name in ["relaxed", "other"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Sized type requirement"));
    }
}
