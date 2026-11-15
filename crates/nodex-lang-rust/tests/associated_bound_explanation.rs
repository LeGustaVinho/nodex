use nodex_lang_rust::lower_source;

#[test]
fn explains_associated_type_bound_on_trait_and_type() {
    let source = "trait T { type Item: Clone; type Other; } struct S; impl T for S { type Item = u32; type Other = (); }";
    let ir = lower_source("src/lib.rs", source);
    let owner = ir.symbols.iter().find(|s| s.name == "T").unwrap();
    let associated = ir.symbols.iter().find(|s| s.name == "Item" && s.parent.as_deref() == Some(owner.id.as_str())).unwrap();
    for symbol in [owner, associated] {
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|i| i.group == "Associated type declaration bound").unwrap();
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "type Item: Clone;");
        assert!(item.text.contains("chosen by each implementation") && item.text.contains("satisfy Clone") && item.text.contains("does not assign a concrete type") && item.text.contains("If Clone resolves"));
    }
    let other = ir.symbols.iter().find(|s| s.name == "Other" && s.parent.as_deref() == Some(owner.id.as_str())).unwrap();
    assert!(!other.rust_contract.iter().flatten().any(|i| i.group == "Associated type declaration bound"));
}
