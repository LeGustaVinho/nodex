use nodex_lang_rust::lower_source;

#[test]
fn explains_combined_lifetime_and_send_requirements_without_resolving_send() {
    let source = "fn inline<'a, T: 'a + Send>(x: T) {} fn where_bound<'a, T>(x: T) where T: Send + 'a {} fn only<'a, T: 'a>(x: T) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["inline", "where_bound"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|i| i.group == "Lifetime and Send requirements").expect("combined bound explanation");
        assert!(item.text.contains("both 'a and Send"));
        assert!(item.text.contains("dropped earlier"));
        assert!(item.text.contains("If Send resolves"));
        assert!(item.text.contains("does not establish"));
    }
    let only = ir.symbols.iter().find(|s| s.name == "only").unwrap();
    assert!(!only.rust_contract.as_ref().into_iter().flatten().any(|i| i.group == "Lifetime and Send requirements"));
}
