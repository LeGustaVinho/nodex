use nodex_lang_rust::lower_source;

#[test]
fn explains_standard_add_spelling_on_type_and_method_without_guessing_other_traits() {
    let source = "struct S; impl std::ops::Add for S { type Output = S; fn add(self, rhs: S) -> S { self } } struct Empty; impl std::ops::Add for Empty {} struct Other; impl Add for Other {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["S", "add", "Empty"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Addition operator implementation").unwrap();
        assert!(item.text.contains("If the path resolves"));
        assert!(item.text.contains("x + y"));
        assert!(item.text.contains("Output"));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].contains("std::ops::Add"));
    }
    let other = ir.symbols.iter().find(|symbol| symbol.name == "Other").unwrap();
    assert!(!other.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Addition operator implementation")));
}
