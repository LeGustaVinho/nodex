use nodex_lang_rust::lower_source;

#[test]
fn mutable_parameter_binding_is_explained_without_confusing_mutable_referent_or_tuple_pattern() {
    let source = "fn f(mut x: i32, mut p: &i32, q: &mut i32, y: i32) {} struct S; impl S { fn m(&self, mut z: i32) {} } fn destructure((mut a, b): (i32, i32)) {}";
    let file = lower_source("src/lib.rs", source);
    for (name, expected) in [("f", 2), ("m", 1)] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Mutable parameter binding").collect();
        assert_eq!(items.len(), expected, "{name}: {:?}", symbol.rust_contract);
        for item in items {
            assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "mut");
            assert!(item.text.contains("mutable local parameter binding"));
            assert!(item.text.contains("caller"));
            assert!(item.text.contains("reference's type"));
            assert!(item.text.contains("does not mean mutation actually occurs"));
        }
    }
    let symbol = file.symbols.iter().find(|symbol| symbol.name == "destructure").unwrap();
    assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Mutable parameter binding"));
}
