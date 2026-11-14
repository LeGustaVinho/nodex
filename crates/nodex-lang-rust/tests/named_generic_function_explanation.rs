use nodex_lang_rust::lower_source;

#[test]
fn explains_named_generic_parameter_in_the_function_contract() {
    let source = "fn identity<T>(x: T) -> T { x } fn same<A, B>(left: A, right: B) {} fn concrete(x: i32) {} fn only_lifetime<'a>(x: &'a str) {}";
    let ir = lower_source("src/lib.rs", source);
    let identity = ir.symbols.iter().find(|s| s.name == "identity").unwrap();
    let fact = identity.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Named generic type parameter").expect("generic parameter explanation");
    assert!(fact.text.contains("placeholder for a concrete type"));
    assert!(fact.text.contains("x: T"));
    assert!(fact.text.contains("same chosen type"));
    assert!(fact.text.contains("infer"));
    assert_eq!(&source[fact.span.start_byte as usize..fact.span.end_byte as usize], "T");
    let same = ir.symbols.iter().find(|s| s.name == "same").unwrap();
    assert_eq!(same.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Named generic type parameter").count(), 2);
    for name in ["concrete", "only_lifetime"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Named generic type parameter"));
    }
}
