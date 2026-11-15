use nodex_lang_rust::lower_source;

#[test]
fn explains_constant_supplied_by_trait_impl_without_conflating_other_constants() {
    let source = "trait T { const N: usize; } struct S; impl T for S { const N: usize = 1; } impl S { const M: usize = 2; } const P: usize = 3;";
    let ir = lower_source("src/lib.rs", source);
    let item = ir.symbols.iter().find(|s| s.name == "N" && s.signature.as_deref().is_some_and(|signature| signature.contains("= 1"))).unwrap();
    let detail = item.rust_contract.iter().flatten().find(|c| c.group == "Trait implementation constant").unwrap();
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "const N: usize = 1;");
    assert!(detail.text.contains("implementation of T for S") && detail.text.contains("<S as T>::N") && detail.text.contains("not establish resolution or validity"));
    for name in ["M", "P"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.iter().flatten().any(|c| c.group == "Trait implementation constant"));
    }
}
