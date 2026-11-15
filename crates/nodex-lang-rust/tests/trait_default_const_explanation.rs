use nodex_lang_rust::lower_source;

#[test]
fn explains_default_and_override_on_trait_and_constant() {
    let source = "trait T { const ID: u32 = 1; const REQUIRED: u32; } struct S; impl T for S { const ID: u32 = 2; const REQUIRED: u32 = 3; } const OUTSIDE: u32 = 1;";
    let file = lower_source("src/lib.rs", source);
    let trait_symbol = file.symbols.iter().find(|symbol| symbol.name == "T").unwrap();
    let item = trait_symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Default associated constant").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "const ID: u32 = 1;");
    assert!(item.text.contains("ID of type u32") && item.text.contains("default expression 1") && item.text.contains("omit") && item.text.contains("override") && item.text.contains("rather than being stored separately"));
    let associated = file.symbols.iter().find(|symbol| symbol.name == "ID" && symbol.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Default associated constant"))).expect("trait constant symbol");
    assert!(associated.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Default associated constant"));
    for symbol in file.symbols.iter().filter(|symbol| symbol.name == "OUTSIDE" || symbol.name == "REQUIRED") {
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Default associated constant"));
    }
}

