use nodex_lang_rust::lower_source;

#[test]
fn plain_inline_is_an_optimization_hint_not_a_guarantee() {
    let source = "#[inline] fn ordinary() {} struct S; impl S { #[inline] fn method(&self) {} } #[inline(always)] fn forced() {} #[inline(never)] fn prevented() {} fn absent() {}";
    let file = lower_source("src/lib.rs", source);
    for name in ["ordinary", "method"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Inlining hint").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[inline]");
        assert!(item.text.contains("compiler can ignore"));
        assert!(item.text.contains("does not guarantee faster"));
        assert!(item.text.contains("#[inline(always)]") && item.text.contains("#[inline(never)]"));
    }
    for name in ["forced", "prevented", "absent"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Inlining hint"));
    }
}
