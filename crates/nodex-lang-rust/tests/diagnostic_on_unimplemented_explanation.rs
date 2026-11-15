use nodex_lang_rust::lower_source;

#[test]
fn on_unimplemented_is_explained_as_a_hint_on_the_annotated_trait() {
    let source = "#[diagnostic::on_unimplemented(message = \"Need {Self}\", label = \"missing\", note = \"implement it\")] #[allow(dead_code)] trait Needed {} #[diagnostic::do_not_recommend] trait Other {} #[cfg_attr(test, diagnostic::on_unimplemented(message = \"conditional\"))] trait Conditional {} trait Plain {}";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|s| s.name == "Needed").unwrap();
    let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Trait diagnostic hint").collect();
    assert_eq!(facts.len(), 1, "{:?}", symbol.rust_contract);
    assert!(source[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize].starts_with("#[diagnostic::on_unimplemented("));
    let text = &facts[0].text;
    assert!(text.contains("trait Needed") && text.contains("may use") && text.contains("message") && text.contains("label") && text.contains("note") && text.contains("{Self}") && text.contains("does not implement the trait"));
    for name in ["Other", "Conditional", "Plain"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Trait diagnostic hint"));
    }
}
