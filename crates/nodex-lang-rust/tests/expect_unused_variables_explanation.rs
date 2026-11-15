use nodex_lang_rust::lower_source;

#[test]
fn explains_expected_unused_variables_without_confusing_allow_or_other_lints() {
    let source = "#[expect(unused_variables)] #[allow(dead_code)] fn a() { let unused = 1; } #[expect(unused_variables,)] fn b() {} #[allow(unused_variables)] fn c() {} #[expect(dead_code)] fn d() {} #[expect(unused_variables, reason = \"why\")] fn e() {}";
    let file = lower_source("src/lib.rs", source);
    for name in ["a", "b"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Expected unused-variable lint").expect("expect contract");
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("#[expect(unused_variables"));
        assert!(item.text.contains("suppress the expected lint"));
        assert!(item.text.contains("unfulfilled_lint_expectations"));
        assert!(item.text.contains("does not prove"));
    }
    for name in ["c", "d", "e"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Expected unused-variable lint"));
    }
}
