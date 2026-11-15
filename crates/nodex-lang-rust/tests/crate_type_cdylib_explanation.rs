use nodex_lang_rust::lower_source;

fn crate_items(source: &str) -> Vec<(String, String)> {
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.flavors.iter().any(|flavor| flavor == "crate-root")).unwrap();
    root.rust_contract.as_ref().into_iter().flatten().filter(|item| item.group == "Native dynamic library crate type")
        .map(|item| (source[item.span.start_byte as usize..item.span.end_byte as usize].to_owned(), item.text.clone())).collect()
}

#[test]
fn explains_written_cdylib_request_without_claiming_build_configuration() {
    let items = crate_items("#![crate_type = \"cdylib\"]\nfn f() {}\n");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "#![crate_type = \"cdylib\"]");
    assert!(items[0].1.contains("crate root") && items[0].1.contains("native dynamic-library") && items[0].1.contains("--crate-type") && items[0].1.contains("does not prove"));
    assert_eq!(crate_items("#![crate_type = r#\"cdylib\"#]\nfn f() {}\n").len(), 1);
    assert!(crate_items("#![crate_type = \"rlib\"]\nfn f() {}\n").is_empty());
    assert!(crate_items("#![cfg_attr(feature = \"x\", crate_type = \"cdylib\")]\nfn f() {}\n").is_empty());
    assert!(crate_items("mod nested { #![crate_type = \"cdylib\"] fn f() {} }").is_empty());
}
