use nodex_lang_rust::lower_source;

#[test]
fn trait_method_opaque_iterator_explains_item_and_implementation_choice() {
    let source = "trait T { fn f(&self) -> impl Iterator<Item = u8>; } fn g() -> impl Iterator<Item = u8> { todo!() } trait U { fn h(&self) -> impl Iterator<Item = u16>; }";
    let ir = lower_source("src/lib.rs", source);
    let f = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let item = f.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Trait method opaque iterator").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "impl Iterator<Item = u8>");
    assert!(item.text.contains("Different trait implementations") && item.text.contains("If Iterator resolves") && item.text.contains("unsigned 8-bit") && item.text.contains("anonymous associated type") && item.text.contains("edition") && item.text.contains("does not establish a particular borrow"));
    for name in ["g", "h"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Trait method opaque iterator"));
    }
}
