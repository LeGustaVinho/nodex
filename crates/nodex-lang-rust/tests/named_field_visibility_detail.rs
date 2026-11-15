use nodex_lang_rust::lower_source;

#[test]
fn named_fields_explain_crate_and_default_visibility_individually() {
    let source = "struct S { pub(crate) x: i32, y: i32, pub z: i32 } struct Tuple(pub(crate) i32);";
    let ir = lower_source("src/lib.rs", source);
    let owner = ir.symbols.iter().find(|s| s.name == "S").unwrap();
    let items: Vec<_> = owner.rust_contract.as_ref().unwrap().iter().filter(|i| i.group == "Named field visibility").collect();
    assert_eq!(items.len(), 2, "{items:?}");
    assert!(items.iter().any(|i| &source[i.span.start_byte as usize..i.span.end_byte as usize] == "pub(crate)" && i.text.contains("current crate") && i.text.contains("other crates")));
    assert!(items.iter().any(|i| &source[i.span.start_byte as usize..i.span.end_byte as usize] == "y" && i.text.contains("declaring module and descendants")));
    assert!(items.iter().all(|i| i.text.contains("own visibility")));
    let tuple = ir.symbols.iter().find(|s| s.name == "Tuple").unwrap();
    assert!(!tuple.rust_contract.iter().flatten().any(|i| i.group == "Named field visibility"));
}
