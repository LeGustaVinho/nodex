use nodex_lang_rust::lower_source;

#[test]
fn inline_always_is_a_hint_and_distinguishes_bodyless_and_other_modes() {
    let source = "#[inline(always)] fn fast() {} trait T { #[inline(always)] fn default(&self) {} #[inline(always)] fn required(&self); } #[inline] fn normal() {} #[inline(never)] fn slow() {} #[cfg_attr(feature = \"x\", inline(always))] fn conditional() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["fast", "default", "required"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Always-inline hint").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[inline(always)]");
        assert!(item.text.contains("hint and the compiler may ignore"));
        assert!(item.text.contains("does not prove that inlining occurred"));
        assert!(item.text.contains("generated poll function"));
        assert!(item.text.contains("cannot be combined with target_feature"));
        if name == "required" { assert!(item.text.contains("no function body")); }
        else { assert!(item.text.contains("function with a body")); }
    }
    for name in ["normal", "slow", "conditional"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Always-inline hint"));
    }
}
