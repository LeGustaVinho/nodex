use nodex_lang_rust::lower_source;

#[test]
fn non_exhaustive_enum_distinguishes_downstream_and_defining_crate() {
    let source = "#[non_exhaustive] #[allow(dead_code)] pub enum Status { Ready, Failed } pub enum Plain { Ready } #[non_exhaustive] pub struct Record { pub value: i32 }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "Status").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Enum may gain variants").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[non_exhaustive]");
    assert!(item.text.contains("future version") && item.text.contains("different crate") && item.text.contains("wildcard `_` arm"));
    assert!(item.text.contains("Inside the defining crate") && item.text.contains("does not add a variant now"));
    for name in ["Plain", "Record"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Enum may gain variants"));
    }
}
