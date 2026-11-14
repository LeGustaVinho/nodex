use nodex_lang_rust::lower_source;

#[test]
fn explains_selected_associated_type_for_each_trait_implementation() {
    let source = "trait T { type Item; } struct S; impl T for S { type Item = u32; } type Alias = u32;";
    let ir = lower_source("src/lib.rs", source);
    let item = ir.symbols.iter().filter(|s| s.name == "Item")
        .find(|s| s.signature.as_deref().is_some_and(|signature| signature.contains("u32")))
        .expect("associated type implementation");
    let explanation = item.rust_contract.iter().flatten()
        .find(|c| c.group == "Associated type definition").expect("visible explanation");
    assert!(explanation.text.contains("implementation of T for S"));
    assert!(explanation.text.contains("type Item = u32"));
    assert!(explanation.text.contains("not a value or a new distinct type"));
    assert_eq!(&source[explanation.span.start_byte as usize..explanation.span.end_byte as usize], "type Item = u32;");
    let alias = ir.symbols.iter().find(|s| s.name == "Alias").unwrap();
    assert!(!alias.rust_contract.iter().flatten().any(|c| c.group == "Associated type definition"));
}
