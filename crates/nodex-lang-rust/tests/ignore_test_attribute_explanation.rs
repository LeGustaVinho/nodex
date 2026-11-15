use nodex_lang_rust::lower_source;

#[test]
fn explains_bare_ignore_conditionally_on_test_harness_without_confusing_related_attributes() {
    let source = "#[test] #[ignore] fn skipped() {} #[ignore] fn not_test() {} #[test] fn normal() {} #[test] #[ignore = \"reason\"] fn reason() {} #[cfg_attr(feature = \"x\", ignore)] #[test] fn conditional() {}";
    let file = lower_source("src/lib.rs", source);
    for name in ["skipped", "not_test"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Ignored test attribute").expect("ignore contract");
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[ignore]");
        assert!(item.text.contains("When the function is a test"));
        assert!(item.text.contains("skipped by a normal test run"));
        assert!(item.text.contains("cargo test -- --ignored"));
        assert!(item.text.contains("custom test harness"));
    }
    for name in ["normal", "reason", "conditional"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Ignored test attribute"));
    }
}
