use nodex_lang_rust::lower_source;

#[test]
fn explains_conjoined_impl_trait_bounds_in_parameter_and_return() {
    let source = "trait Drawable {} fn both(x: impl Drawable + Send) -> impl Drawable + Send { x } fn single(x: impl Drawable) {} fn dynamic(x: &dyn Drawable) {}";
    let ir = lower_source("src/lib.rs", source);
    let both = ir.symbols.iter().find(|s| s.name == "both").unwrap();
    let items: Vec<_> = both.rust_contract.as_ref().unwrap().iter().filter(|i| i.group == "Combined impl Trait bounds").collect();
    assert_eq!(items.len(), 2, "{items:?}");
    assert!(items.iter().any(|i| i.text.contains("caller supplies")));
    assert!(items.iter().any(|i| i.text.contains("implementation chooses") && i.text.contains("same concrete type")));
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "impl Drawable + Send");
        assert!(item.text.contains("every listed bound"));
        assert!(item.text.contains("If Send refers"));
    }
    for name in ["single", "dynamic"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|i| i.group == "Combined impl Trait bounds"));
    }
}
