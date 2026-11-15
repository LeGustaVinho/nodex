use nodex_lang_rust::lower_source;

#[test]
fn inferred_bound_uses_the_only_input_reference_without_claiming_a_borrow() {
    let source = "trait T {} struct S; impl T for S {} fn f(x: &str) -> impl T + '_ { S } fn named<'a>(x: &'a str) -> impl T + 'a { S } fn many(x: &str, y: &str) -> impl T + '_ { S } fn plain(x: &str) -> impl T { S }";
    let file = lower_source("src/lib.rs", source);
    let f = file.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let item = f.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Inferred lifetime bound on an opaque return").unwrap_or_else(|| panic!("{:?}", f.rust_contract));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "'_");
    assert!(item.text.contains("only reference parameter"));
    assert!(item.text.contains("outlives bound"));
    assert!(item.text.contains("does not prove that the return actually borrows"));
    assert!(item.text.contains("does not imply heap allocation or dynamic dispatch"));
    for name in ["named", "many", "plain"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Inferred lifetime bound on an opaque return"));
    }
}
