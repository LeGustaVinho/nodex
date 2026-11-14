use nodex_lang_rust::lower_source;

#[test]
fn required_associated_const_is_explained_in_trait_and_its_own_contract() {
    let text = "trait T { const N: usize; const DEFAULT: usize = 5; } const OTHER: usize = 7;";
    let ir = lower_source("src/lib.rs", text);
    for name in ["T", "N"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter()
            .find(|item| item.group == "Required associated constant").unwrap();
        assert!(item.text.contains("each implementation"));
        assert!(item.text.contains("N with type usize"));
        assert!(item.text.contains("provides no value"));
        assert!(item.text.contains("rather than stored separately in each instance"));
        assert_eq!(&text[item.span.start_byte as usize..item.span.end_byte as usize], "const N: usize;");
    }
    for name in ["DEFAULT", "OTHER"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Required associated constant")));
    }
}
