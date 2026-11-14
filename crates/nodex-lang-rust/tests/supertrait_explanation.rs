#[test]
fn exposes_supertrait_requirement_on_trait_symbol() {
    let source = "trait Parent {} trait Child: Parent {} trait Other {}";
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let child = ir.symbols.iter().find(|symbol| symbol.name == "Child").unwrap();
    let items = child.rust_contract.as_ref().unwrap();
    assert!(items.iter().any(|item| item.group == "Trait requirements"
        && item.text.contains("implementing Child to implement Parent")));
    let other = ir.symbols.iter().find(|symbol| symbol.name == "Other").unwrap();
    assert!(!other.rust_contract.as_ref().is_some_and(|items|
        items.iter().any(|item| item.group == "Trait requirements")));
}
