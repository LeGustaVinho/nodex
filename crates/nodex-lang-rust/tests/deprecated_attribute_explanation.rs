use nodex_lang_rust::lower_source;

#[test]
fn explains_bare_deprecated_without_confusing_note_form_or_lint_controls() {
    let source = "#[deprecated] #[inline] fn old() {} #[deprecated(note = \"use newer\")] fn noted() {} #[allow(deprecated)] fn allowed() {} fn plain() {}";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "old").unwrap();
    let fact = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Deprecated item").unwrap();
    assert_eq!(&source[fact.span.start_byte as usize..fact.span.end_byte as usize], "#[deprecated]");
    assert!(fact.text.contains("normally a warning") && fact.text.contains("lint settings") && fact.text.contains("does not itself remove") && fact.text.contains("no reason or replacement"));
    for name in ["noted", "allowed", "plain"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Deprecated item"), "{name}");
    }
}
