use nodex_lang_rust::lower_source;
#[test]
fn direct_test_attribute_is_explained_on_its_function_without_claiming_indirect_attributes() {
    let source = "#[test] // comment\n#[should_panic] fn sample() {} fn ordinary() {} #[tokio::test] async fn async_test() {} #[cfg_attr(test, test)] fn conditional() {} #[test(foo)] fn invalid() {}";
    let ir = lower_source("src/lib.rs", source);
    let sample = ir.symbols.iter().find(|symbol| symbol.name == "sample").unwrap();
    let items: Vec<_> = sample.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Built-in test attribute").collect();
    assert_eq!(items.len(), 1, "{:?}", sample.rust_contract);
    let item = items[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[test]");
    assert!(item.text.contains("built-in test harness") && item.text.contains("cargo test") && item.text.contains("does not execute") && item.text.contains("#[should_panic]") && item.text.contains("#[ignore]"));
    for name in ["ordinary", "async_test", "conditional", "invalid"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Built-in test attribute"), "{name}");
    }
}
