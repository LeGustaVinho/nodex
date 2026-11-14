use nodex_lang_rust::lower_source;

#[test]
fn explains_combined_trait_bounds_without_claiming_unresolved_names_are_std() {
    let ir = lower_source("src/lib.rs", "trait T: Send + Sync {} trait One: Send {} trait Other: Read + Write {}");
    let get = |name: &str| ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
    let details = get("T").rust_contract.as_ref().unwrap();
    let item = details.iter().find(|item| item.group == "Multiple supertraits").unwrap();
    for expected in ["+ means all", "Send, Sync", "If these names refer", "shared references", "name resolution"] {
        assert!(item.text.contains(expected), "missing {expected:?}: {}", item.text);
    }
    assert!(!get("One").rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Multiple supertraits")));
    let other = get("Other").rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Multiple supertraits").unwrap();
    assert!(!other.text.contains("Rust's standard traits"));
}
