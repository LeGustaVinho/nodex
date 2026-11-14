use nodex_lang_rust::lower_source;

#[test]
fn shared_receiver_explained_on_inherent_and_trait_method_symbols() {
    let source = "struct S; impl S { fn read(&self) {} fn edit(&mut self) {} fn take(self) {} } trait T { fn read(&self); }";
    let ir = lower_source("src/lib.rs", source);
    let methods = ir.symbols.iter().filter(|s| s.name == "read").collect::<Vec<_>>();
    assert_eq!(methods.len(), 2);
    for method in methods {
        let item = method.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Shared receiver").unwrap();
        assert!(item.text.contains("caller keeps ownership"));
        assert!(item.text.contains("interior mutability"));
        assert!(item.text.contains("multiple shared references"));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "&self");
    }
    for method in ir.symbols.iter().filter(|s| s.name == "edit" || s.name == "take") {
        assert!(!method.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Shared receiver"));
    }
}
