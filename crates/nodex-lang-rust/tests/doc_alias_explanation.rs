use nodex_lang_rust::lower_source;

#[test]
fn doc_alias_preserves_each_written_string_and_describes_search_without_renaming() {
    let source = r###"
        #[doc(alias = "old_name")] pub fn current() {}
        #[doc(alias="one", alias=r#"raw_alias"#)] pub struct S;
        #[doc(alias = "old\u{5f}name")] fn escaped() {}
        #[doc(hidden)] #[doc(alias = "secret")] fn hidden() {}
        #[cfg_attr(test, doc(alias = "conditional"))] fn conditional() {}
        #[doc(alias("list_a", "list_b"))] fn alternate_form() {}
        #[doc = "alias = not_an_alias"] fn documentation_only() {}
        #[doc(alias = concat!("old", "_name"))] fn generated_alias() {}
    "###;
    let ir = lower_source("src/lib.rs", source);
    for (name, expected) in [("current", vec!["\"old_name\""]), ("S", vec!["\"one\"", "r#\"raw_alias\"#"]), ("escaped", vec!["\"old\\u{5f}name\""]), ("hidden", vec!["\"secret\""])] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Documentation search alias").collect();
        assert_eq!(items.len(), expected.len(), "{name}: {:?}", symbol.rust_contract);
        for written in expected {
            let item = items.iter().find(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize] == written).unwrap_or_else(|| panic!("{name}: {written:?}: {items:?}"));
            assert!(item.text.contains(written), "{name}: {}", item.text);
            for phrase in ["documentation search index", "string-literal rules", "does not rename", "Rust identifier or path", "visibility settings", "does not prove"] {
                assert!(item.text.contains(phrase), "{name}: {}", item.text);
            }
        }
    }
    for name in ["conditional", "alternate_form", "documentation_only", "generated_alias"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Documentation search alias"), "{name}");
    }
}
