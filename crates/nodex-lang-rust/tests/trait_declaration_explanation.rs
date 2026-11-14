use nodex_lang_rust::lower_source;

#[test]
fn trait_explains_required_and_default_methods_without_confusing_structs() {
    let text = "trait Drawable { fn draw(&self); fn reset(&self) {} } struct Other;";
    let ir = lower_source("src/lib.rs", text);
    let drawable = ir.symbols.iter().find(|symbol| symbol.name == "Drawable").unwrap();
    let item = drawable.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Trait").unwrap();
    assert!(item.text.contains("set of behavior that types can implement"));
    assert!(item.text.contains("draw has no body here, so each implementation must provide it"));
    assert!(item.text.contains("&self parameter borrows"));
    assert!(item.text.contains("reset has a default body here"));
    let other = ir.symbols.iter().find(|symbol| symbol.name == "Other").unwrap();
    assert!(!other.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Trait")));
}
