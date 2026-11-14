use nodex_lang_rust::lower_source;

#[test]
fn required_associated_type_explains_declaration_and_implementation() {
    let source = "trait T { type Item; } struct S; impl T for S { type Item = u32; } type Plain = u8;";
    let ir = lower_source("src/lib.rs", source);
    let trait_symbol = ir.symbols.iter().find(|symbol| symbol.name == "T").unwrap();
    let declared = ir.symbols.iter().find(|symbol|
        symbol.name == "Item" && symbol.parent.as_deref() == Some(trait_symbol.id.as_str())
    ).unwrap();
    let item = declared.rust_contract.as_ref().unwrap().iter()
        .find(|item| item.group == "Associated type syntax").unwrap();
    assert!(item.text.contains("does not create a value"));
    assert!(item.text.contains("each implementation must supply"));
    assert!(item.text.contains("<Concrete as T>::Item"));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "type Item;");
    let plain = ir.symbols.iter().find(|symbol| symbol.name == "Plain").unwrap();
    assert!(!plain.rust_contract.iter().flatten().any(|item| item.group == "Associated type syntax"));
    let implementation = ir.symbols.iter().find(|symbol|
        symbol.name == "Item" && symbol.parent.as_deref() != Some(trait_symbol.id.as_str())
    ).unwrap();
    assert!(!implementation.rust_contract.iter().flatten().any(|item| item.group == "Associated type syntax"));
}
