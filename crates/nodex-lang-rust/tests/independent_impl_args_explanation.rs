use nodex_lang_rust::lower_source;

#[test]
fn two_impl_trait_parameters_are_independent_even_with_identical_bounds() {
    let source = "trait Trait {} fn f(_: impl Trait, _: impl Trait) {} fn different(a: impl A, b: impl B) {} fn one(_: impl Trait) {} fn output(_: impl Trait) -> impl Trait { todo!() } fn named<T: Trait>(a: T, b: T) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "different"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Independent impl Trait arguments").unwrap();
        for phrase in ["separate anonymous generic type parameter", "different concrete types", "each type satisfies its own", "may also be the same", "does not require", "fn f<T: Trait>", "depend on resolution"] { assert!(item.text.contains(phrase), "{name}: {}", item.text); }
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with('('));
    }
    let f = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    assert_eq!(f.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Generic parameter type").count(), 2);
    for name in ["one", "output", "named"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Independent impl Trait arguments"), "{name}");
    }
}
