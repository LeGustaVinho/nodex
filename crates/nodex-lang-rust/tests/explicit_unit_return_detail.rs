use nodex_lang_rust::lower_source;

#[test]
fn explicit_unit_return_explains_implicit_equivalent_and_async() {
    let source = "fn f() -> () {} async fn later() -> () {} fn inferred() {} fn value() -> i32 { 1 }";
    let file = lower_source("src/lib.rs", source);
    for name in ["f", "later"] {
        let symbol = file.symbols.iter().find(|s| s.name == name).unwrap();
        let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|i| i.group == "Explicit unit return").collect();
        assert_eq!(facts.len(), 1, "{:?}", symbol.rust_contract);
        assert_eq!(&source[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], "()");
        for part in ["Omitting the return annotation", "one value", "never", "normally"] { assert!(facts[0].text.contains(part)); }
        if name == "later" { assert!(facts[0].text.contains("future")); }
    }
    for name in ["inferred", "value"] {
        let symbol = file.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|i| i.group == "Explicit unit return"));
    }
}
