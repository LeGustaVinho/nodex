#[test]
fn new_convention_is_explained_on_the_function_contract() {
    let ir = nodex_lang_rust::lower_source("src/lib.rs", "struct S; impl S { fn new() -> Self { Self } fn make() -> Self { Self } }");
    let new = ir.symbols.iter().find(|symbol| symbol.name == "new").unwrap();
    assert!(new.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Associated function" && item.text.contains("convention") && item.text.contains("body determines")));
    let make = ir.symbols.iter().find(|symbol| symbol.name == "make").unwrap();
    assert!(!make.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Associated function")));
}
