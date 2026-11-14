use nodex_lang_rust::lower_source;

#[test]
fn union_symbol_explains_shared_storage_and_unsafe_reads() {
    let ir = lower_source("src/lib.rs", "union Bits { i: u32, f: f32 } struct Separate { i: u32, f: f32 }");
    let union = ir.symbols.iter().find(|symbol| symbol.name == "Bits").unwrap();
    let detail = union.rust_contract.as_ref().unwrap().iter()
        .find(|item| item.group == "Union").unwrap();
    assert!(detail.text.contains("i: u32"));
    assert!(detail.text.contains("f: f32"));
    assert!(detail.text.contains("share storage"));
    assert!(detail.text.contains("does not track"));
    assert!(detail.text.contains("requires unsafe"));
    assert!(detail.text.contains("valid value"));
    let separate = ir.symbols.iter().find(|symbol| symbol.name == "Separate").unwrap();
    assert!(!separate.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Union"));
}
