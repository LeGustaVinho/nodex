use nodex_lang_rust::lower_source;

#[test]
fn explains_outer_doc_value_preserving_written_literal_and_excludes_other_doc_forms() {
    let source = "#[doc = \"Documentation\"] #[allow(dead_code)] struct S; #[doc = \"A B\\nC\"] fn f() {} struct P { #[doc = \"Field doc\"] x: i32 } #[doc(hidden)] struct Hidden; #[doc(alias = \"other\")] struct Alias;";
    let file = lower_source("src/lib.rs", source);
    for (name, literal) in [("S", "\"Documentation\""), ("f", "\"A B\\nC\""), ("x", "\"Field doc\"")] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Documentation value attribute").expect("doc contract");
        assert!(item.text.contains(literal), "{}", item.text);
        assert!(item.text.contains("following item"));
        assert!(item.text.contains("Markdown"));
        assert!(item.text.contains("not executed"));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("#[doc ="));
    }
    for name in ["Hidden", "Alias"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Documentation value attribute"));
    }
}
