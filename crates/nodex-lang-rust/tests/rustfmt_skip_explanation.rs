use nodex_lang_rust::lower_source;

#[test]
fn direct_rustfmt_skip_is_explained_on_its_item_not_other_attributes() {
    let source = "#[rustfmt::skip]\n#[inline]\nfn spaced() { let  x=1; }\n#[rustfmt::skip] struct Table { x: i32 }\n#[cfg_attr(test, rustfmt::skip)] fn conditional() {}\n#[rustfmt::skip::macros(html)] fn macro_skip() {}\n// #[rustfmt::skip]\nfn comment_only() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["spaced", "Table"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Skip rustfmt formatting").collect();
        assert_eq!(facts.len(), 1, "{name}: {:?}", symbol.rust_contract);
        let item = facts[0];
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[rustfmt::skip]");
        assert!(item.text.contains("when that formatter is run"));
        assert!(item.text.contains("not a Rust instruction executed at runtime"));
        assert!(item.text.contains("does not make the compiler skip the item"));
        assert!(item.text.contains("cfg_attr"));
    }
    for name in ["conditional", "macro_skip", "comment_only"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Skip rustfmt formatting"), "{name}: {:?}", symbol.rust_contract);
    }
}
