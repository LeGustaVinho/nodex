use nodex_lang_rust::lower_source;

#[test]
fn explains_associated_type_equality_without_assuming_the_trait_resolves_to_std() {
    let source = "fn consume<T, I: Iterator<Item = T>>(it: I) {} fn other<T: Custom<Item = u32>>() {} fn plain<T: Iterator>() {}";
    let ir = lower_source("src/lib.rs", source);
    let consume = ir.symbols.iter().find(|symbol| symbol.name == "consume").unwrap();
    let item = consume.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Associated type equality").unwrap();
    assert!(item.text.contains("exactly T"));
    assert!(item.text.contains("does not assign a runtime value"));
    assert!(item.text.contains("If Iterator resolves"));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "Item = T");
    let other = ir.symbols.iter().find(|symbol| symbol.name == "other").unwrap();
    let item = other.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Associated type equality").unwrap();
    assert!(item.text.contains("exactly u32"));
    assert!(!item.text.contains("standard iterator"));
    let plain = ir.symbols.iter().find(|symbol| symbol.name == "plain").unwrap();
    assert!(!plain.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Associated type equality"));
}
