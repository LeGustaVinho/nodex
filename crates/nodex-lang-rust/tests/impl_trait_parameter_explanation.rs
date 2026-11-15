use nodex_lang_rust::lower_source;

#[test]
fn parameter_impl_trait_explains_caller_choice_and_independence() {
    let source = "trait Drawable {} fn show(a: impl Drawable, b: &impl Drawable) {} trait Api { fn draw(x: impl Drawable); } fn produce() -> impl Drawable { todo!() }";
    let ir = lower_source("src/lib.rs", source);
    let show = ir.symbols.iter().find(|symbol| symbol.name == "show").unwrap();
    let items: Vec<_> = show.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Generic parameter type").collect();
    assert_eq!(items.len(), 2, "{:?}", show.rust_contract);
    for item in items {
        assert!(item.text.contains("caller supplies a concrete type"));
        assert!(item.text.contains("independent"));
        assert!(item.text.contains("anonymous generic type parameter"));
        assert!(item.text.contains("cannot be explicitly named as a generic argument"));
        assert!(item.text.contains("compiler checks"));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "impl Drawable");
    }
    let draw = ir.symbols.iter().find(|symbol| symbol.name == "draw").unwrap();
    assert!(draw.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Generic parameter type"));
    let produce = ir.symbols.iter().find(|symbol| symbol.name == "produce").unwrap();
    assert!(!produce.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Generic parameter type"));
}
