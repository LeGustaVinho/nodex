use nodex_lang_rust::lower_source;

#[test]
fn explains_bare_should_panic_without_matching_expected_message_form() {
    let source = "#[test] #[should_panic] fn fails() { panic!(); } #[test] #[should_panic(expected = \"boom\")] fn specific() { panic!(\"boom\"); } #[test] fn ordinary() {}";
    let ir = lower_source("src/lib.rs", source);
    let function = ir.symbols.iter().find(|symbol| symbol.name == "fails").unwrap();
    let item = function.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Expected panic").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[should_panic]");
    assert!(item.text.contains("passes if it panics") && item.text.contains("fails if it returns normally"));
    assert!(item.text.contains("does not itself cause a panic") && item.text.contains("does not require a particular panic message"));
    for name in ["specific", "ordinary"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Expected panic"));
    }
}
