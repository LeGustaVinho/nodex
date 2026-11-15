use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn static_link_attribute_is_visible_on_module_and_foreign_declaration() {
    let source = "#[link(name = \"x\", kind = \"static\")] unsafe extern \"C\" { fn foo(); } #[link(name = \"y\", kind = \"dylib\")] unsafe extern \"C\" { fn dyn_fn(); } mod inner { #[link(kind = \"static\", name = \"z\")] unsafe extern \"C\" {} }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.parent.is_none() && s.kind == SymbolKind::Module).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Native static library link").collect();
    assert_eq!(items.len(), 1, "{:?}", root.rust_contract);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "#[link(name = \"x\", kind = \"static\")]");
    assert!(items[0].text.contains("name = \"x\""));
    assert!(items[0].text.contains("does not prove that it was found"));
    assert!(items[0].text.contains("not a Rust variable's mutability or lifetime"));
    let foo = ir.symbols.iter().find(|s| s.name == "foo").unwrap();
    assert!(foo.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Native static library link" && item.text.contains("name = \"x\"")));
    let dyn_fn = ir.symbols.iter().find(|s| s.name == "dyn_fn").unwrap();
    assert!(!dyn_fn.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Native static library link"));
    let inner = ir.symbols.iter().find(|s| s.name == "inner").unwrap();
    assert!(inner.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Native static library link" && item.text.contains("name = \"z\"")));
}
