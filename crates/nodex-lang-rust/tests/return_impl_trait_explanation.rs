use nodex_lang_rust::lower_source;

#[test]
fn explains_opaque_return_type_on_functions_and_trait_methods() {
    let source = "trait Drawable {} struct Circle; impl Drawable for Circle {} fn make() -> impl Drawable { Circle } fn accepts(x: impl Drawable) {} trait Factory { fn make(&self) -> impl Drawable; } fn boxed() -> Box<dyn Drawable> { todo!() }";
    let ir = lower_source("src/lib.rs", source);
    let makes: Vec<_> = ir.symbols.iter().filter(|symbol| symbol.name == "make").collect();
    assert_eq!(makes.len(), 2);
    for symbol in makes {
        let fact = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Opaque return type").unwrap();
        assert_eq!(&source[fact.span.start_byte as usize..fact.span.end_byte as usize], "impl Drawable");
        assert!(fact.text.contains("one concrete type"));
        assert!(fact.text.contains("all return paths"));
        assert!(fact.text.contains("does not imply a heap allocation or dynamic dispatch"));
    }
    for symbol in ir.symbols.iter().filter(|symbol| symbol.name == "accepts" || symbol.name == "boxed") {
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Opaque return type"));
    }
}
