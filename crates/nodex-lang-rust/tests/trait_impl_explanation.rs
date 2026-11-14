use nodex_lang_rust::lower_source;

#[test]
fn explains_trait_impl_on_type_and_method_without_claiming_inherent_impl_is_a_trait() {
    let source = "trait Drawable { fn draw(&self); } struct Player; impl Drawable for Player { fn draw(&self) {} } impl Player { fn tick(&self) {} } struct Empty; impl Drawable for Empty {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["Player", "draw", "Empty"] {
        let symbols: Vec<_> = ir.symbols.iter().filter(|symbol| symbol.name == name).collect();
        assert!(symbols.iter().any(|symbol| symbol.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Trait implementation" && item.text.contains("Drawable") && item.text.contains("does not create")))), "missing explanation for {name}");
    }
    let tick = ir.symbols.iter().find(|symbol| symbol.name == "tick").unwrap();
    assert!(!tick.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Trait implementation")));
}
