use nodex_lang_rust::lower_source;

#[test]
fn explains_type_alias_without_calling_it_a_new_type() {
    let source = "type Alias = Vec<u8>; mod inner { type Count = u32; } struct Wrapper(u32);";
    let ir = lower_source("src/lib.rs", source);
    let alias = ir.symbols.iter().find(|symbol| symbol.name == "Alias").unwrap();
    let item = alias.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Type alias").unwrap();
    assert!(item.text.contains("existing type Vec<u8>"));
    assert!(item.text.contains("does not create a distinct type"));
    assert!(item.text.contains("If Vec refers to Rust's standard Vec"));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "type Alias = Vec<u8>;");
    let count = ir.symbols.iter().find(|symbol| symbol.name == "Count").unwrap();
    assert!(count.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Type alias"));
    let wrapper = ir.symbols.iter().find(|symbol| symbol.name == "Wrapper").unwrap();
    assert!(!wrapper.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Type alias")));
}
