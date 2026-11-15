use nodex_lang_rust::lower_source;

#[test]
fn cold_is_a_hint_on_the_annotated_function_with_a_body() {
    let source = "#[cold] #[inline] fn slow() {} trait T { #[cold] fn fallback(&self) {} #[cold] fn required(&self); } #[cfg_attr(test, cold)] fn conditional() {} #[cold] struct InvalidPosition; fn plain() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["slow", "fallback"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Infrequent function hint").collect();
        assert_eq!(items.len(), 1, "{}: {:?}", name, symbol.rust_contract);
        assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "#[cold]");
        assert!(items[0].text.contains("expected to be uncommon") && items[0].text.contains("optimization hint") && items[0].text.contains("does not guarantee") && items[0].text.contains("can still be called normally"));
    }
    for name in ["required", "conditional", "InvalidPosition", "plain"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Infrequent function hint"));
    }
}
