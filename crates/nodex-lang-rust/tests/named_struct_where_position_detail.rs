use nodex_lang_rust::lower_source;

#[test]
fn named_struct_where_clause_explains_position_and_bound() {
    let source = "struct S<T> where T: Copy { x: T } struct Pair<T>(T, T) where T: Copy; struct Plain { x: i32 }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "S").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Where clause on named-field struct").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "where T: Copy");
    for phrase in ["after the struct name", "before the fields in braces", "if `Copy` resolves", "does not copy a field", "entire struct Copy", "after their parenthesized field list", "resolution and checking"] { assert!(item.text.contains(phrase), "{}", item.text); }
    for name in ["Pair", "Plain"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Where clause on named-field struct"), "{name}");
    }
}
