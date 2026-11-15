use nodex_lang_rust::lower_source;

fn crate_items(source: &str) -> Vec<(String, String)> {
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.flavors.iter().any(|flavor| flavor == "crate-root")).unwrap();
    root.rust_contract.as_ref().into_iter().flatten().filter(|item| item.group == "Type length limit")
        .map(|item| (source[item.span.start_byte as usize..item.span.end_byte as usize].to_owned(), item.text.clone())).collect()
}

#[test]
fn explains_limit_and_conditional_compiler_enforcement() {
    let items = crate_items("#![type_length_limit = \"1000000\"]\nfn f() {}\n");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "#![type_length_limit = \"1000000\"]");
    for expected in ["1000000", "monomorphization", "nightly", "-Zenforce-type-length-limit", "does not prove", "first occurrence"] {
        assert!(items[0].1.contains(expected), "missing {expected}");
    }
    assert_eq!(crate_items("#![type_length_limit = r#\"0\"#]\nfn f() {}\n").len(), 1);
    assert!(crate_items("#![recursion_limit = \"1000000\"]\nfn f() {}\n").is_empty());
    assert!(crate_items("#![cfg_attr(feature = \"x\", type_length_limit = \"4\")]\nfn f() {}\n").is_empty());
    assert!(crate_items("mod nested { #![type_length_limit = \"4\"] fn f() {} }").is_empty());
}
