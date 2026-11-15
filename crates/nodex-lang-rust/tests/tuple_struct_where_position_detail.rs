use nodex_lang_rust::lower_source;

#[test]
fn trailing_where_applies_to_generic_tuple_struct_not_its_last_field() {
    let source = "struct Pair<T>(T, T) where T: Copy; struct One<T>(T) where T: Clone; struct Named<T> where T: Copy { a: T } struct Plain<T>(T);";
    let ir = lower_source("src/lib.rs", source);
    for name in ["Pair", "One"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Tuple struct trailing where clause").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        for phrase in ["tuple struct", "positional fields", "after the parenthesized fields", "whole struct", "not just the final field", "does not construct a value"] { assert!(item.text.contains(phrase), "{name}: {}", item.text); }
        if name == "Pair" {
            assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "where T: Copy");
            for phrase in ["T, T", "If `Copy` resolves", "does not automatically make the struct itself `Copy`", "does not establish trait resolution"] { assert!(item.text.contains(phrase), "{}", item.text); }
        }
    }
    for name in ["Named", "Plain"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Tuple struct trailing where clause"));
    }
}
