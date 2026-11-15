use nodex_lang_rust::lower_source;

#[test]
fn explains_required_associated_function_returning_the_implementor() {
    let source = "trait T { fn f() -> Self; fn with_arg(x: i32) -> Self; fn defaulted() -> Self { loop {} } async fn async_f() -> Self; fn other() -> i32; } fn outside() -> i32 { 0 }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Required trait function returning its implementor").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "Self");
    for expected in ["Trait T requires", "no default body", "concrete type implementing", "no `self` receiver", "<ConcreteType as T>::f()", "does not tell us"] { assert!(item.text.contains(expected), "missing {expected}"); }
    for name in ["with_arg", "defaulted", "async_f", "other", "outside"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Required trait function returning its implementor"), "{name}");
    }
}
