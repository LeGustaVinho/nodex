use nodex_lang_rust::lower_source;

#[test]
fn crate_visibility_on_individual_tuple_fields_is_explained() {
    let source = "struct Wrapper(pub(crate) u32); pub struct Pair(u8, pub(crate) u32, pub u8); struct Private(u32); struct Named { x: u32 }";
    let file = lower_source("src/lib.rs", source);
    for (name, index) in [("Wrapper", "index 0"), ("Pair", "index 1")] {
        let symbol = file.symbols.iter().find(|s| s.name == name).unwrap();
        let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|i| i.group == "Crate-visible tuple struct field").collect();
        assert_eq!(facts.len(), 1, "{:?}", symbol.rust_contract);
        assert_eq!(&source[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], "pub(crate)");
        for part in [index, "u32", "current crate", "another crate", "constructor", "Conditional compilation"] { assert!(facts[0].text.contains(part), "{}", facts[0].text); }
    }
    for name in ["Private", "Named"] {
        let symbol = file.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|i| i.group == "Crate-visible tuple struct field"));
    }
}
